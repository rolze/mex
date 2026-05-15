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
cargo run             # run against ../.mex.db (auto-detected relative path)
```

Release build (faster startup and rendering):

```bash
cargo build --release
./target/release/mex
```
