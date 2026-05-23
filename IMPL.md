# mex — Implementation Notes

The source code is the source of truth for *how*. This document records the
durable design rules — *what* must hold across changes and *why*.

## Overview

`mex` is a Rust + [Ratatui](https://github.com/ratatui-org/ratatui) terminal
UI for browsing a media library backed by SQLite. Split-pane layout: filterable
file list on the left, image preview on the right.

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

## Module map

```
mex/src/
├── main.rs      Process bootstrap: terminal setup, channels, threads, event loop
├── app.rs       Application state, key handling, navigation, command execution
├── ui.rs        Pure rendering: layout, list, preview pane, overlays
├── db.rs        SQLite queries — accepts a borrowed Connection, never opens one
├── import.rs    Media ingestion pipeline (date/slug derivation, dedup, counters)
├── player.rs    Video/audio playback delegation
└── config.rs    Config file + env var resolution
```

One module = one concern. Cross-module calls go through narrow function
boundaries; no module reaches into another's state.

---

## Architectural patterns

### Single-threaded UI, message-passing for everything else

The main thread owns all UI state and is the only thread that mutates `App`.
Background work (image encoding, import, filesystem deletion, slug removal)
runs on dedicated threads that communicate via mpsc channels. **No shared
mutable state** between UI and workers — every result flows back as a typed
message that the event loop drains and applies.

### Caller-owned resources

Library-style functions (notably in `db.rs`) accept the resources they need
as borrowed parameters. They never acquire them internally. This applies to:

- SQLite connections (`&Connection` / `&mut Connection`)
- Channel senders for progress reporting
- Filesystem roots and config paths

The caller decides resource scope and lifetime; the callee is pure logic over
borrowed inputs. This keeps hot paths free of per-call setup cost and makes
tests trivial — pass a fixture, assert the result.

### Tiered caches keyed by cost layer

Caching is layered by what each layer *avoids*, not by data identity:

| Layer | Avoids | Invalidation trigger |
|---|---|---|
| Decoded-image cache | Disk read + image decode | Capacity eviction |
| Encoded-protocol cache | Resize + terminal-escape encoding | Path change |
| Terminal-side image store (Kitty) | Re-transmitting pixels | Terminal resize |

Each layer has its own key and its own invalidation rule. A layer is *only*
invalidated when the work it caches genuinely needs redoing — never as a
side-effect of unrelated UI transitions (e.g. closing a preview pane does
not invalidate the encoded protocol; the same file may be reopened instantly).

### Monotonic IDs for async result staleness

Any work dispatched to a background thread carries a monotonic request ID.
When the response returns, the main thread compares the ID against the
current expected ID and drops stale responses. This is the only correct way
to handle a user who navigates faster than the worker can encode.

### Cache invalidation is explicit, never incidental

Resetting cached state (encoded protocol, current path, loading flag) happens
only on events that genuinely invalidate the context: filter change, file
deletion, navigation to a different file. UI transitions that do not change
the underlying content (toggling pane visibility, opening menus) must not
touch caches.

---

## SOLID-adjacent principles

- **Single responsibility** — one module per concern; see module map. A file
  that grows a second concern is a signal to split.
- **Dependency inversion** — high-level code depends on abstractions, not
  concrete resource acquisition. `db` functions take `&Connection`, not a
  path; `App` takes a constructed `Connection`, not a factory.
- **Open / closed** — terminal image protocol is an enum dispatched at one
  site; adding a new protocol means adding a variant, not editing call sites.
- **Interface segregation (Rust flavour)** — read paths take `&T`; only the
  minimum number of functions take `&mut T` (currently: transaction-using
  writes only). Tests and callers benefit from the loosest bound that works.

---

## Rust non-negotiables

These rules are load-bearing. Violating them re-introduces classes of bugs
the codebase has already paid to eliminate.

- **No hidden resource acquisition in library functions.** A function that
  takes `&str` and opens a `Connection` inside is forbidden. Caller passes
  `&Connection` (or `&mut Connection`) and owns its lifetime.
- **`&T` for reads, `&mut T` only where the type's API forces it.** In
  practice this means `&Connection` for queries, `&mut Connection` only when
  starting a transaction. Pick the loosest bound that compiles.
- **Respect `Send` boundaries.** Non-`Send` resources (e.g. `rusqlite::
  Connection`) live in exactly one thread. Threads that need their own
  resource open it once at thread start and reuse it for the thread's
  lifetime — never per iteration of a loop.
- **Cross-thread coordination uses mpsc, not `Arc<Mutex<_>>`.** Shared mutable
  state is the wrong primitive when one side is the UI event loop. Reach for
  a channel first; only consider locks if message-passing genuinely cannot
  express the requirement.
- **No `Connection::open` inside loops.** Open once outside the loop. The
  same rule generalises: amortise expensive setup across the natural batch.
- **Errors propagate with `?`.** Library functions return `Result`; UI code
  surfaces errors as status messages. `.unwrap()` / `.expect()` are
  acceptable only in tests and in `main` for genuinely fatal startup
  failures.
- **Tests use in-memory or temp-dir SQLite, never the user's DB.** Test
  helpers return live `Connection` objects so tests can both mutate and
  verify against the same handle — no `drop` + reopen dance.
- **Behavioural tests assert invariants, not implementation.** Tests count
  encode dispatches, observe cache hits, and verify state transitions —
  they do not assert on private fields or specific call ordering beyond
  what the contract requires.

---

## Terminal image protocol

`mex` queries the terminal at startup (before entering the alternate screen)
to negotiate an image protocol. `MEX_PROTOCOL` overrides detection:

```sh
MEX_PROTOCOL=kitty   ./mex
MEX_PROTOCOL=sixel   ./mex
MEX_PROTOCOL=iterm2  ./mex
MEX_PROTOCOL=halfblocks ./mex
```

The active protocol appears in the preview title (e.g. `Preview [kitty]`).

| Protocol | Terminals |
|---|---|
| Kitty | kitty, WezTerm, Ghostty, foot, Konsole (partial) |
| Sixel | xterm -ti vt340, mlterm, WezTerm, Windows Terminal |
| iTerm2 | iTerm2, WezTerm, Hyper |
| Halfblocks | All terminals (fallback) |

### Cost model

Image rendering cost splits into three independently-cacheable stages:
**disk read + decode** → **resize + terminal-encode** → **terminal I/O**.

The first two are CPU/memory work and are cached in-process. The third —
writing bytes to the terminal — is unavoidable per frame and dominates when
the protocol is halfblocks (one multi-byte cell per pixel pair, O(W × H)
cells per redraw). Single-sequence protocols (Sixel, Kitty, iTerm2) collapse
this to one escape sequence; Kitty additionally caches the pixel data
terminal-side, so subsequent renders reference an ID rather than retransmit.

The in-process caches do not help with terminal-I/O cost. Choose Kitty when
available for the lowest reopen cost.
