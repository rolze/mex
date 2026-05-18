# Agent Instructions

## UC document sync

Every UC document (`spec/UC-XX.md`) is the source of truth for its feature.

**Rule: every implementation change must be reflected in the corresponding UC document in the same commit.**

- If you add, remove, or change a behaviour, update the UC doc to match.
- UC docs must be brief and concise — describe *what is implemented*, not aspirations.
- Remove outdated details immediately; do not leave stale text.
- If a change spans multiple UCs, update all affected docs.

This keeps the UC files useful as a living reference rather than historical artefacts.

## End-to-end / visual testing with tmux

Use tmux to run the TUI and observe real terminal output:

```bash
# Always open a fresh window — never reuse a dirty one
tmux kill-window -t mex-test 2>/dev/null
tmux new-window -n mex-test
sleep 1
tmux send-keys -t mex-test "cd /path/to/mex && ./target/debug/mex 2>/dev/null" Enter
sleep 6   # wait for startup

# Capture what the terminal shows
tmux capture-pane -t mex-test -p | tail -10

# Send a key and re-capture
tmux send-keys -t mex-test "/" && sleep 0.3
tmux capture-pane -t mex-test -p | tail -4

# Clean up when done
tmux kill-window -t mex-test
```

**Rules:**
- Always kill the old window before creating a new one (`tmux kill-window -t mex-test 2>/dev/null`).
- Start mex with the pre-built binary (`./target/debug/mex`), not `cargo run`, to avoid keystrokes being swallowed by cargo's startup output.
- Use generous `sleep` delays after startup (≥ 5 s) and after key sends (≥ 0.2 s) before capturing.
- If the filter bar shows shell command text, the binary was not running yet when keys were sent — kill and retry with a fresh window.
