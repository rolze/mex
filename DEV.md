# mex — Developer Setup

## Prerequisites

### Rust toolchain

Install via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# follow the prompts, then reload your shell:
source "$HOME/.cargo/env"
```

Verify:

```bash
cargo --version   # e.g. cargo 1.78.0
```

### chafa (optional — image preview)

`chafa` converts images to Unicode/ANSI art for terminal display.
The right pane falls back to metadata-only if `chafa` is not found.

```bash
# Debian / Ubuntu / WSL
sudo apt install chafa

# Arch
sudo pacman -S chafa

# macOS (Homebrew)
brew install chafa
```

## Build & Run

```bash
cd mex/
cargo build           # debug build
cargo run             # run against .mex.db (auto-detected relative path)
```

Release build (faster startup and rendering):

```bash
cargo build --release
./target/release/mex
```

## Release pipeline

### CI (every push / PR to `main`)

`.github/workflows/ci.yml` — runs `cargo test` on `ubuntu-latest`.  
Triggers automatically; no manual steps needed.

### Publishing a release

1. Merge everything into `main` and push.
2. Tag the commit with a semver tag:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. `.github/workflows/release.yml` triggers automatically and:
   - Builds a fully static binary (`x86_64-unknown-linux-musl`, SQLite bundled)
   - Strips and names it `mex-linux-x86_64`
   - Creates a GitHub Release with auto-generated notes and attaches the binary

The release appears at `https://github.com/rolze/mex/releases`.

### Build the musl binary locally

```bash
rustup target add x86_64-unknown-linux-musl
sudo apt install musl-tools        # Debian / Ubuntu / WSL
cd mex/
cargo build --release --target x86_64-unknown-linux-musl
strip target/x86_64-unknown-linux-musl/release/mex
```

---

## One-off commands

### Backfill partial hashes

Computes and stores partial hashes for files that are missing them in the database:

```bash
cargo run --bin migrate-partial-hashes -- [--db .mex.db] [--root /media/root]
```

Both flags are optional; defaults are auto-detected from the current directory.
