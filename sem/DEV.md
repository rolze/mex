# sem — Development Guide

## System requirements

### Runtime

| Requirement | Minimum | Notes |
|---|---|---|
| Linux | any modern distro | Wayland or X11 |
| GTK4 runtime | 4.8 | usually `libgtk-4-1` |
| libadwaita runtime | 1.4 | usually `libadwaita-1-0` |

sem is a native Linux GUI application. It does not run in headless/SSH
environments without a forwarded display.

### Build

| Requirement | Minimum | Package (Debian/Ubuntu) |
|---|---|---|
| Rust toolchain | stable (1.75+) | `rustup` |
| GTK4 dev headers | 4.8 | `libgtk-4-dev` |
| libadwaita dev headers | 1.4 | `libadwaita-1-dev` |
| pkg-config | any | `pkg-config` |

No other system libraries are required for the default build. Thumbnail
generation uses the pure-Rust `image` crate (no `libvips`, `libjpeg`, or
`libwebp` needed).

Install build deps on Debian/Ubuntu:

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config
```

### Optional: libvips backend

Enable the `vips` Cargo feature for faster thumbnailing with wider format
support (HEIF, AVIF, TIFF, RAW, etc.). Requires libvips 8.16+ at build and
runtime.

Install the extra dep on Ubuntu 26.04:

```sh
sudo apt install libvips-dev
```

Build with the feature:

```sh
cargo build --features vips
cargo build --release --features vips
```

The default build (without `--features vips`) continues to work on any system.

---

## Build

```sh
cd sem

# Debug build (faster compile, slower binary)
cargo build
# binary: sem/target/debug/sem

# Debug build with libvips backend (requires libvips-dev)
cargo build --features vips

# Release build
cargo build --release
# binary: sem/target/release/sem

# Release build with libvips backend
cargo build --release --features vips
```

### Make available to mex

mex looks for `sem` on `$PATH`. The recommended approach:

```sh
# Install to ~/.cargo/bin (already on PATH if you use rustup)
cargo install --path .
```

Or prepend the debug build location without installing:

```sh
export PATH="/path/to/mex-repo/sem/target/debug:$PATH"
```

---

## Version pinning

The GTK stack crate versions are pinned to match the system libraries:

| Crate | Version | System lib | Why pinned |
|---|---|---|---|
| `gtk4` | `"0.8"` | GTK 4.14 | `"0.9"` requires GTK 4.16+ |
| `libadwaita` | `"0.6"` | libadwaita 1.5 | `"0.7"` requires libadwaita 1.6+ |

If you upgrade the system GTK stack, bump these crate versions accordingly.
The `v4_8` / `v1_5` feature flags unlock API available since those minor versions.

**Do not upgrade to `gtk4 = "0.9"` or `libadwaita = "0.7"` on Ubuntu 24.04** —
the packaged library versions are too old.

---

## Known API pitfalls

### `glib::MainContext::channel` does not exist

`gtk4 = "0.8"` pulls in `glib = "0.19"`. The `MainContext::channel` helper
was present in glib 0.18 and removed in 0.19. Any example or Stack Overflow
answer using it will not compile.

**Replacement pattern** for worker-thread → GTK-thread communication:

```rust
let (tx, rx) = std::sync::mpsc::channel::<MyMsg>();

std::thread::spawn(move || {
    tx.send(compute()).ok();
});

glib::idle_add_local(move || {
    match rx.try_recv() {
        Ok(msg) => { /* update UI */ glib::ControlFlow::Continue }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    }
});
```

### `libvips` Rust crate — historical property-name bug

The `libvips` Rust crate was generated against libvips 8.16+, which renamed
`import-profile` / `export-profile` to `input-profile` / `output-profile`.
On systems with libvips ≤ 8.15, every call to `thumbnail_with_opts` fails at
runtime with `"no property named 'input-profile'"`.

sem's optional `vips` feature requires libvips ≥ 8.16. Ubuntu 26.04 ships
8.18 so the issue does not arise there. On older systems use the default
`image`-crate build (no `--features vips`).

### `gtk4::ContentFit` requires feature flag

`ContentFit` was added in GTK 4.8. Without `features = ["v4_8"]` in
`Cargo.toml`, the enum does not exist at compile time, even on systems
with a newer GTK installed.

---

## Visual / end-to-end testing

sem is a GUI application; automated testing requires a display. Use tmux
to run sem against real images and observe the output:

```sh
# Build first
cd sem && cargo build

# Open a tmux window
tmux kill-window -t sem-test 2>/dev/null
tmux new-window -n sem-test

# Single-image test
tmux send-keys -t sem-test \
  "sem /path/to/image.jpg --tags 'holiday,2023'" Enter
sleep 3

# Grid test — write a manifest and open
printf '/path/to/a.jpg\t\n/path/to/b.png\ttag\n' > /tmp/test.txt
tmux send-keys -t sem-test \
  "sem --files /tmp/test.txt --cache-dir /tmp/sem-cache" Enter
sleep 5

# Check the cache was populated
ls -lh /tmp/sem-cache/

# Clean up
tmux kill-window -t sem-test
```

---

## Project layout

```
sem/
├── Cargo.toml          crate manifest
├── Cargo.lock          locked deps (committed)
├── README.md           user-facing overview
├── ARCHITECTURE.md     technical design notes (this repo)
├── DEV.md              this file
├── .gitignore          excludes target/
└── src/
    ├── main.rs         CLI, mode dispatch, manifest parsing
    ├── cache.rs        thumbnail cache (SHA-256 key, JPEG via image crate)
    └── window.rs       GTK4 UI (single + grid modes, progressive loading)
```

---

## Related specs

- [`spec/UC-15.md`](../spec/UC-15.md) — full feature spec for sem & mex integration
- [`ARCHITECTURE.md`](../ARCHITECTURE.md) — mex architecture (the TUI side)
