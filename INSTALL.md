# Installing mex

## Native Linux

```bash
cargo build --release
# copy target/release/mex somewhere on your PATH
```

No additional dependencies beyond Rust stable.

---

## Windows / WSL2

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
