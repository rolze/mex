# mex (Prototype) — Architecture Decision Log

This log captures implementation-specific details for the legacy `mex/` prototype.
For overall project rules and the 4-layer architecture model, see `doc/ARCHITECTURE.md`.

## Module map (Legacy)

The prototype `mex/` does not fully adhere to the 4-layer model. Its structure is:

```
mex/src/
├── main.rs      Process bootstrap: terminal setup, channels, threads, event loop
├── app.rs       Application state, key handling, navigation, command execution
│               App holds cohesive sub-structs: FilterState, CommandState,
│               ImageState, MpvState, and four background-worker structs.
├── ui.rs        Pure rendering: layout, list, preview pane, overlays
├── db.rs        SQLite queries
├── import.rs    Media ingestion pipeline (date/slug derivation, dedup, counters)
├── player.rs    Video/audio playback delegation
└── config.rs    Config file + env var resolution
```

## Tiered Caches

Caching in the prototype is layered by what each layer *avoids*:

| Layer | Avoids | Invalidation trigger |
|---|---|---|
| Decoded-image cache | Disk read + image decode | LRU capacity eviction (cap = 30) |
| Encoded-protocol cache | Resize + terminal-escape encoding | Path change |
| Terminal-side image store (Kitty) | Re-transmitting pixels | Terminal resize |

## Terminal Image Protocol

`mex` queries the terminal at startup to negotiate an image protocol. `MEX_PROTOCOL` overrides detection.

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
the protocol is halfblocks. Single-sequence protocols (Sixel, Kitty, iTerm2) collapse
this to one escape sequence; Kitty additionally caches the pixel data
terminal-side, so subsequent renders reference an ID rather than retransmit.
