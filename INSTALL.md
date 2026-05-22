# Installing sem and mex

## Quick install (Linux x86_64)

```bash
curl -fsSL https://raw.githubusercontent.com/rolze/mex/main/install.sh | bash
```

The script:
- Downloads the latest `sem` and `mex` binaries from the [GitHub releases page](https://github.com/rolze/mex/releases/latest)
- Auto-detects `libvips` and picks the faster `sem-vips` variant if available
- Installs to `/usr/local/bin` (system-wide) or `~/.local/bin` (user-only) — whichever is writable
- Checks that required runtime libraries are present and prints the exact `apt install` command if any are missing

> **Requirements**: Linux x86_64, `curl`

---

## mex

### Native Linux

```bash
cargo build --release
# copy target/release/mex somewhere on your PATH
```

No additional dependencies beyond Rust stable.

### Pre-built binary (Linux x86_64)

Download `mex-linux-x86_64` from the [latest release](https://github.com/rolze/mex/releases/latest),
make it executable, and put it on your PATH:

```bash
chmod +x mex-linux-x86_64
sudo mv mex-linux-x86_64 /usr/local/bin/mex
```

This is a fully static musl binary — no runtime dependencies.

---

## sem

`sem` is the companion GTK4 image viewer launched by `mex`. It requires GTK4 and libadwaita at
runtime (dynamically linked).

### Choosing a binary

Two pre-built variants are provided:

| Binary | Thumbnail backend | Extra runtime dep |
|--------|-------------------|-------------------|
| `sem-linux-x86_64` | pure Rust (`image` crate) | — |
| `sem-linux-x86_64-vips` | libvips (faster for large libraries) | `libvips42` |

### Pre-built binary (Linux x86_64)

**1 — Install runtime libraries**

For `sem-linux-x86_64`:
```bash
sudo apt install libgtk-4-1 libadwaita-1-0
```

For `sem-linux-x86_64-vips` (additionally):
```bash
# Ubuntu 22.04 / Debian 12
sudo apt install libvips42
# Ubuntu 24.04+
sudo apt install libvips42t64
```

**2 — Download and install**

```bash
chmod +x sem-linux-x86_64          # or sem-linux-x86_64-vips
sudo mv sem-linux-x86_64 /usr/local/bin/sem
```

### Build from source

```bash
cd sem/

# Default (image crate backend)
cargo build --release

# With libvips backend (requires libvips-dev)
cargo build --release --features vips
```

---

## mex — Windows / WSL2

mex runs in WSL2 and controls a **native Windows mpv.exe** for video playback.
This requires a one-time bridge setup so WSL can talk to mpv's Windows named pipe.

### 1 — Install mpv for Windows

Download from https://mpv.io/installation/ and install it. The default path assumed by mex is:

```
C:\Program Files\MPV Player\mpv.exe
```

Other locations are supported — mex probes `cmd.exe /c where mpv` and a list of common paths
at startup, then saves the result to `~/.config/mex/config.toml`. You can also set it manually:

```toml
# ~/.config/mex/config.toml
mpv_path = /mnt/c/Program Files/MPV Player/mpv.exe
```

### 2 — Install socat in WSL

```bash
sudo apt install socat
```

### 3 — Install npiperelay.exe

`npiperelay.exe` bridges the WSL Unix socket to the Windows named pipe that mpv creates.

1. Download the latest release from https://github.com/jstarks/npiperelay/releases
2. Place `npiperelay.exe` somewhere on your `$PATH` inside WSL, for example:

```bash
mkdir -p ~/.local/bin
cp /mnt/c/Users/<you>/Downloads/npiperelay.exe ~/.local/bin/
# ensure ~/.local/bin is on your PATH
```

### How the bridge works

When you press `p` on a video file, mex:

1. Starts `socat` listening on a Unix socket (`/tmp/mex-mpv.sock`)
2. Spawns `mpv.exe --input-ipc-server=mex-mpv` (creates `\\.\pipe\mex-mpv` on Windows)
3. Each IPC connection is forwarded by `npiperelay.exe` from the Unix socket to the named pipe

All playback controls (`s` pause/resume, `j`/`k` next/prev, media keys) and the live status box
work identically to native Linux once the bridge is up.

### Troubleshooting

| Symptom | Likely cause |
|---------|-------------|
| `p` does nothing | Check `mpv_path` in `~/.config/mex/config.toml` |
| `view: could not start socat bridge` | `socat` is not installed — run `apt install socat` |
| `socat bridge socket did not appear` | `npiperelay.exe` is not on `$PATH` |
| Video opens but wrong file / garbled path | Should not happen after the fix; report a bug |

You can verify the bridge manually:

```bash
# Should print the mpv version if everything is wired up
(echo '{"command":["get_property","mpv-version"]}'; sleep 1) \
  | socat - UNIX-CONNECT:/tmp/mex-mpv.sock
```
