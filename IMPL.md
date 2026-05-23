# mex — Implementation Notes

## Overview

`mex` is a Rust + [Ratatui](https://github.com/ratatui-org/ratatui) terminal UI for browsing a
media library backed by a SQLite database (`.mex.db`). It runs entirely in the terminal with a
split-pane layout: a filterable file list on the left and an image preview on the right.

```
┌──────────────────────────┬──────────────────────────────┐
│  mex — 49640 files       │  Preview [kitty]             │
│  2022/ foo.jpg   photo   │  File: 2022/2022-01-…        │
│▶ 2023/ bar.png   travel  │  Date: 2023-05-12            │
│  2023/ baz.jpg   —       │  Ext:  jpg                   │
│  …                       │  Tags: travel, holiday       │
│                          │  ┌──────────────────────┐   │
│                          │  │  <image>             │   │
│                          │  └──────────────────────┘   │
├──────────────────────────┴──────────────────────────────┤
│  /filter_                                               │
└─────────────────────────────────────────────────────────┘
```

---

## Caching layers

There are **two independent caches** with very different cost profiles:

### 1 — DynamicImage cache (disk I/O avoidance)

| Field | `App::image_cache: HashMap<PathBuf, DynamicImage>` |
|---|---|
| Capacity | 30 entries (CACHE_MAX), evicts ~⅓ when full |
| Saves | Disk read + JPEG/PNG decode (~66 ms for a typical photo) |
| Does NOT save | Terminal-encode step (resize + pixel → escape sequences) |
| Eviction | Simple: remove arbitrary keys until below CACHE_MAX |

When navigating away from an image and back:
- **Cache miss** (first visit): 66 ms read + decode → insert cache → dispatch encode
- **Cache hit** (second visit): 2–3 ms clone → dispatch encode (25× faster disk path)

### 2 — ThreadProtocol / encoded-protocol cache (encode avoidance)

| Field | `App::image_state: ThreadProtocol` · `App::current_image_path` |
|---|---|
| Saves | The full resize + encode cycle in the background thread |
| Key | `current_image_path` — if the path matches, skip everything |

When closing and reopening the preview **on the same file**:
- `toggle_preview()` sets `preview_open = false` only — **does NOT call `empty_protocol()`**
- `ThreadProtocol::inner` keeps the already-encoded `StatefulProtocol` alive in memory
- On reopen: `refresh_image()` early-returns in < 1 µs; `StatefulImage` renders from `inner` directly
- `encode_dispatch_count` does **not** increase (verified by unit tests)

> **Important**: `empty_protocol()` must only be called when the image context is genuinely
> invalidated (e.g. filter change), not on every preview close.

---

## Background encoding thread

```
main thread                          encoder thread
────────────────────────────────     ──────────────────────────
refresh_image()
  replace_protocol(StatefulProtocol)  ← inner = Some(proto), no send yet
  current_image_path = Some(path)
  is_loading = true

StatefulImage::render()              → rx_worker.recv()
  needs_resize? → Some(size)            request.resize_encode()
  inner.take() → ResizeRequest          (resize pixels + encode to
  tx_worker.send(request)  ──────────►  escape sequences)
  inner = None (in-flight)              tx_result.send(response)
                           ◄──────────
run_loop: rx_result.try_recv()
  app.on_encode_done(response)
  inner = Some(encoded_proto)
  is_loading = false
```

- Poll interval: 16 ms (~60 fps)
- Loading spinner shown while `is_loading = true`
- `update_resized_protocol()` uses a monotonic `id` counter to discard stale responses
  (e.g. if the user navigates quickly and a new encode was dispatched before the old one finished)

---

## Terminal I/O — the real rendering bottleneck

Even with both caches working correctly, there is an unavoidable cost when the preview pane
**first appears** (or reappears after being hidden): ratatui must write the image data to the
terminal.

The cost depends entirely on the **image protocol** negotiated with the terminal:

| Protocol | How it works | Cells written | Reopen cost |
|---|---|---|---|
| **Halfblocks** | One Unicode `▄`/`▀` per 2 pixels | O(W × H) cells | ~150 ms for a 80×40 pane |
| **Sixel** | Single escape sequence encodes entire image | 1 sequence | ~5–10 ms |
| **Kitty** | Single escape sequence, terminal caches image | 1 sequence (+ ID lookup on reopen) | ~1–2 ms |
| **iTerm2** | Base64-encoded PNG in a single sequence | 1 sequence | ~5–10 ms |

### Why halfblocks feels slow on reopen

Ratatui uses double-buffering: it diffs the previous frame buffer against the new one and only
sends changed cells. When the preview is hidden, the image area is occupied by list characters.
When reopened, **every cell** in the image area changes (list char → halfblock char). For a
80-column × 40-row pane that is ~3 200 cells — each a multi-byte UTF-8 sequence — sent in a
single frame.

**The encode cache is irrelevant for this cost** — the expensive part is the terminal I/O, not
the Rust CPU work.

### Kitty protocol advantages

With the Kitty Graphics Protocol, the terminal stores the image and assigns it an ID. Subsequent
renders reference the ID rather than re-transmitting pixel data. This makes:
- First display: ~same as Sixel (one base64 sequence)
- Reopen (same image, same size): near-instant (terminal already has it)
- Terminal resize: one new sequence at the new size

---

## Protocol selection

`mex` queries the terminal at startup (before entering the alternate screen) via
`Picker::from_query_stdio()`. If detection fails or returns halfblocks, the protocol can be
overridden with the `MEX_PROTOCOL` environment variable:

```sh
MEX_PROTOCOL=kitty   ./mex   # force Kitty Graphics Protocol
MEX_PROTOCOL=sixel   ./mex   # force Sixel
MEX_PROTOCOL=iterm2  ./mex   # force iTerm2 inline images
MEX_PROTOCOL=halfblocks ./mex  # explicit halfblocks (default fallback)
```

The active protocol is shown in the preview pane title: `Preview [kitty]`.

**Terminal compatibility:**

| Protocol | Terminals |
|---|---|
| Kitty | kitty, WezTerm, Ghostty, foot, Konsole (partial) |
| Sixel | xterm -ti vt340, mlterm, WezTerm, Windows Terminal |
| iTerm2 | iTerm2, WezTerm, Hyper |
| Halfblocks | All terminals (fallback) |

---

## Module map

```
mex/src/
├── main.rs    Terminal setup, mpsc channels, bg encoder thread, event loop
├── app.rs     App state, navigation, filter, image loading/caching logic
├── ui.rs      Ratatui layout, list rendering, preview pane, spinner overlay
└── db.rs      SQLite query (media + tags JOIN, HAVING-based filter)
```

---

## Performance benchmarks (unit tests)

Run with `cargo test -- --nocapture` from `mex/`:

| Test | Result | What it proves |
|---|---|---|
| `first_open_dispatches_once` | 1 encode | Cold open works |
| `same_path_no_second_dispatch` | 0 extra encodes | Path check short-circuits |
| `close_reopen_no_redispatch` | 0 extra encodes | Protocol kept alive on close |
| `same_path_refresh_is_sub_millisecond` | < 1 ms | Early-return is instant |
| `cache_hit_faster_than_cold_read` | ~2.6 ms vs ~66 ms (25×) | DynamicImage cache works |
| `navigate_away_and_back_dispatches` | 3 encodes for 3 images | Navigate correctly invalidates |
| `dynimage_is_cached_after_first_load` | cache len == 2 | No premature eviction |
| `filter_clears_image_state` | path = None | Filter properly resets state |

---

## SQLite connection model

### Problem (before 2026-05-23)

Every public function in `db.rs` called `Connection::open(db_path)` at entry and dropped it on
return. The empty-trash background thread called `delete_trashed_from_fs(…, slice::from_ref(id))`
inside a per-file loop — N deletions → N connection open/close cycles. `PRAGMA` settings were
re-applied on every call.

### Fix

All 16 public db functions now accept a borrowed connection rather than a path string:

```rust
// before
pub fn load_files(db_path: &str) -> Result<Vec<MediaFile>> { … }

// after
pub fn load_files(conn: &Connection) -> Result<Vec<MediaFile>> { … }
```

`assign_tag` is the only function that needs `&mut Connection` because it calls
`conn.transaction()`, which takes `&mut self`. All other functions only call `execute`,
`query_row`, `prepare` — all `&self` in rusqlite.

### Connection ownership

| Scope | Owner | Lifetime |
|---|---|---|
| Main thread | `App::conn: rusqlite::Connection` | Process lifetime |
| `main()` bootstrap | Local `conn` in `fn main` | Passed into `App::new()` |
| Background threads | Local `let conn = Connection::open(&db_path)?` | Thread lifetime |

`Connection` is not `Send`, so background threads cannot borrow `App::conn`. Each thread clones
`self.db_path: String` and opens its own connection once at thread start.

`db_path: String` is retained in `App` for two reasons:
1. Background threads clone it to open their own connections.
2. The thumbnail-cache path is computed as `format!("{}.cache", self.db_path)`.

### Test helpers

- `test_conn() -> Connection` — returns `Connection::open_in_memory().unwrap()` for tests that
  construct an `App` but don't exercise the database.
- `make_tag_db()` now returns `(PathBuf, rusqlite::Connection)` — the open file-backed connection
  is reused directly in tests rather than drop-and-reopen.
- `db.rs` internal tests (for `fix_date`, `load_files`) were updated to reuse the setup
  connection for both mutations and post-condition queries, eliminating the drop/reopen pattern.

### Import thread was already correct

`confirm_import` opened one connection (`let mut conn = rusqlite::Connection::open(&db_path)?`)
and passed `&mut conn` to `execute_import`. No change was needed there.
