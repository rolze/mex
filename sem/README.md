# sem — Image Viewer

A lightweight companion image viewer for [mex](../README.md), written in **Rust + GTK4/libadwaita**. No editing, no DAM — just a fast, clean window for looking at images.

## What it does

- **Opens** a single image in a native GTK4 window, scaled to fit.
- **Shows** the filename and tags below the image — passed directly from mex, no database access needed.
- **Stays out of the way** — mex remains fully interactive while sem is open.

## Usage

```
sem <path> [--tags <comma-separated-tags>]
```

```sh
# Open an image standalone
sem /mnt/photos/2023/2023-05-12_sunset.jpg

# Open with tags (as mex does it)
sem /mnt/photos/2023/2023-05-12_sunset.jpg --tags "travel,holiday"
```

Press **Escape** or close the window to quit.

## mex integration

In mex, press `p` on any image file — sem opens in a new window with the file's tags pre-loaded. Videos still go to mpv. See [spec/UC-15.md](../spec/UC-15.md).

## Build

Requires GTK4 ≥ 4.8 and libadwaita ≥ 1.4 development headers.

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
| Error handling | `anyhow` |
