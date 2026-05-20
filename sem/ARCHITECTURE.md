# sem — Architecture Notes

## Overview

`sem` is a standalone GTK4 + libadwaita image viewer launched by `mex` as a detached subprocess.
It has two modes selected at startup by the CLI arguments:

```
sem <path> [--tags t1,t2]          # single-image mode
sem --files <manifest> --cache-dir <dir>   # grid mode
```

Neither mode blocks the mex TUI — `mex` spawns sem detached and returns immediately.

---

## Module map

```
sem/src/
├── main.rs     CLI parsing (clap), mode dispatch, manifest parsing
├── cache.rs    Thumbnail cache: SHA-256 key, JPEG generation via `image` crate
└── window.rs   GTK4 UI: run_single / run_grid, progressive loading, navigation
```

---

## Single-image mode

Widget tree:

```
ApplicationWindow (libadwaita)
└── ToolbarView
    ├── HeaderBar          title = filename
    └── Box (vertical)
        ├── Picture        ContentFit::Contain, vexpand+hexpand
        └── Label          "<filename>  ·  tag1, tag2"
```

`gtk4::Picture::for_filename` lets GTK load and display the image natively —
no Rust-side decode at all. The `ContentFit::Contain` constraint requires
`gtk4 = { features = ["v4_8"] }` (added in GTK 4.8).

Escape key → `window.close()` via an `EventControllerKey` attached to the window.

---

## Grid mode

### Widget tree

```
ApplicationWindow (libadwaita)
└── ToolbarView
    ├── HeaderBar
    │   └── Button (go-previous-symbolic)  — hidden until single view
    └── Stack
        ├── "grid" page
        │   └── ScrolledWindow
        │       └── FlowBox
        │           └── [FlowBoxChild →] Box (vertical)  × N
        │               ├── Picture   256×256, ContentFit::Contain
        │               └── Label     filename, ellipsized
        └── "single" page
            └── Box (vertical)
                ├── Picture   ContentFit::Contain, vexpand+hexpand
                └── Label     caption
```

The `Stack` named pages `"grid"` / `"single"` are swapped by:
- `FlowBox::connect_child_activated` — single click on a cell
- Back button `connect_clicked`
- `EventControllerKey` for Escape (single → grid; grid → close)

The single-view `Picture` and `Label` are pre-built once and reused across all
thumbnail activations — only `set_file` and `set_text` are called on each click.

### Progressive thumbnail loading

Thumbnails are generated in a dedicated background thread and pushed to the
GTK main thread via `std::sync::mpsc`:

```
main thread                        worker thread
──────────────────────────────     ──────────────────────────────
build_grid_window()
  FlowBox cells created             thread::spawn:
  (grey placeholders)               for each entry:
  mpsc::channel() → (tx, rx)          ensure_thumbnail(path, cache)
  thread::spawn(tx)  ──────────►      tx.send((idx, Some(thumb_path)))
  glib::idle_add_local(rx):            or
    loop: rx.try_recv()                tx.send((idx, None))  // error

glib idle callback fires:
  Ok((idx, Some(path))) →
    cell_pictures[idx].set_file(…)
  Empty → ControlFlow::Continue
  Disconnected → ControlFlow::Break
```

**Why `std::sync::mpsc` + `glib::idle_add_local` instead of `glib::MainContext::channel`:**

`glib::MainContext::channel` existed in glib 0.18 but was removed in glib 0.19.
`gtk4 = "0.8"` pulls in glib 0.19, so the channel API is unavailable.
The replacement is `std::sync::mpsc::channel` (no `Send` constraint on the closure)
combined with `glib::idle_add_local` which polls `try_recv()` on every GTK idle cycle.

The idle callback signature is `FnMut() -> ControlFlow + 'static`. It returns:
- `ControlFlow::Continue` — re-queue for the next idle cycle (more work or empty)
- `ControlFlow::Break` — deregister (sender dropped → all work done)

---

## Thumbnail cache

### Key

```
SHA-256(path_bytes ++ \0 ++ file_size_le64 ++ mtime_secs_le64)
first 16 hex chars + ".jpg"
```

Using both size and mtime ensures the cache is invalidated if the source file
is modified in-place without renaming.

### Generation

`image::open(source).thumbnail(256, 256).into_rgb8().save_with_format(…, Jpeg)`

`DynamicImage::thumbnail` uses the `image` crate's built-in Lanczos-like filter
and constrains both dimensions to ≤ 256 px while preserving aspect ratio.
The result is converted to `Rgb8` before saving (avoids JPEG encoder rejecting
images with alpha channels).

### Why not libvips

The original implementation used the `libvips = "2.0.2"` Rust crate.
It failed at runtime on every call with:

```
thumbnail: no property named `input-profile'
```

**Root cause:** `libvips` 2.0.2 was generated against a newer version of the
C library that renamed the thumbnail properties from `import-profile` /
`export-profile` to `input-profile` / `output-profile`. The system has
libvips 8.15.1 which still uses the old names. The Rust binding *always*
emits the property names as C varargs even when the values are `None` / `NULL`,
so every call fails regardless of whether profiles are actually needed.

The `image` crate is pure Rust, has no version skew with system libraries,
and is entirely sufficient for 256 px thumbnail generation.

---

## Dependency versions

The GTK stack versions are tightly constrained by what is installed on the system:

| System library | Version | Rust crate | Constraint |
|---|---|---|---|
| GTK4 | 4.14.5 | `gtk4 = "0.8"` | `features = ["v4_8"]` required for `ContentFit` |
| libadwaita | 1.5.0 | `libadwaita = "0.6"` | `"0.7"` requires ≥ 1.6 (not available) |
| glib (transitive) | — | `glib = "0.19"` | `glib::MainContext::channel` removed in 0.19 |

The `v4_8` feature flag unlocks `gtk4::ContentFit`, used for `Picture::set_content_fit`.
Without it the enum is unavailable at compile time even though the runtime supports it.

---

## mex integration

mex writes a tab-separated manifest to `/tmp/mex-sem-<micros>.txt`:

```
/abs/path/a.jpg\ttag1,tag2
/abs/path/b.png\t
```

Then spawns sem detached:

```sh
sem --files /tmp/mex-sem-<micros>.txt --cache-dir <db_path>.cache/
```

`cache_dir` is derived as `<db_path>.cache` — a sibling directory to `.mex.db`.
mex does not delete the manifest; the OS cleans `/tmp` eventually.

The dispatch logic in `mex/src/app.rs::view_selected()`:
- ≥ 2 images selected → `open_selection_in_sem()` (grid mode)
- exactly 1 image / cursor on image → `open_in_sem()` (single mode)
- video → mpv (UC-13)
- other → status error

---

## Application ID

`io.github.rolze.sem` — used by GApplication for session management and
D-Bus registration. Both `run_single` and `run_grid` use the same ID,
meaning a second sem invocation while one is already running will activate
the existing instance rather than opening a new window. This is standard
GApplication behaviour and is acceptable for the current use case.
