## UC-12 · Open external image viewer / video player

**Actor:** User  
**Goal:** Open the current file in the system default viewer/player via `Ctrl+O`.

---

### Flow

1. User positions the cursor on a file.
2. User presses `Ctrl+O` (or `Shift+Enter` on terminals with Kitty keyboard protocol support).
3. mex builds the absolute path (`target_root + target_path`) and spawns `xdg-open <path>` detached — the viewer/player opens in the background and mex keeps running.
4. On error (file missing, `xdg-open` not found) a status message is shown at the bottom of the screen.

### Notes

- Multi-selection is ignored; always opens the cursor file.
- The TUI is not suspended; mex remains fully interactive while the external app is open.
- `Ctrl+O` works in all terminals. `Shift+Enter` is an alias that requires a terminal with Kitty keyboard protocol support (e.g. kitty, WezTerm, Ghostty, foot); standard terminals such as GNOME Terminal cannot distinguish it from plain `Enter`.
