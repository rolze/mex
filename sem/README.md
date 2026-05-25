# sem — Image Viewer

A lightweight companion image viewer for [mex](../README.md), written in **Rust + GTK4/libadwaita**. No editing, no DAM — just a fast, clean window for looking at images.

## What it does

- **Single-image mode** — opens one image scaled to fit, with filename and tags shown below.
- **Grid mode** — shows a 256 px thumbnail grid for 2+ selected images; click any thumbnail to view full-size. Thumbnails are cached on disk (JPEG) so repeat opens are instant.
- **Stays out of the way** — mex remains fully interactive while sem is open.

## Usage

```sh
# Single image (press p on a cursor image in mex)
sem /mnt/photos/sunset.jpg --tags "travel,holiday"

# Grid view (press p with 2+ images selected in mex)
sem --files /tmp/mex-sem-1234.txt --cache-dir /mnt/photos/.mex.db.cache/
```

Manifest file format (`--files`): tab-separated `path\ttags`, one image per line.

Press **Escape** to go back (grid → close; single-from-grid → back to grid).

## mex integration

In mex:
- `p` on a cursor image → single-image view in sem.
- `p` with ≥ 2 images selected → grid view in sem.
- Videos still go to mpv.

See [mex/spec/UC-15-sem-viewer.md](../mex/spec/UC-15-sem-viewer.md).

## Build

Requires GTK4 ≥ 4.8 and libadwaita ≥ 1.4 development headers. No other system libraries needed.

```sh
# Install to ~/.cargo/bin (recommended for use with mex)
cd sem
cargo install --path .

# Or build a local debug binary
cargo build
# binary at sem/target/debug/sem
```

## Stack

| Concern | Crate |
|---------|-------|
| UI | `gtk4`, `libadwaita` |
| CLI args | `clap` |
| Thumbnails | `image` (pure Rust) |
| Cache keys | `sha2`, `hex` |
| Error handling | `anyhow` |
