# Testing — Sem & Mex

## Automated tests

```bash
cd mex && cargo test            # unit + integration tests
cd mex && cargo clippy -- -D warnings   # lint — zero warnings policy
cd mex && cargo fmt --check     # format check — must pass before commit
```

All three must pass before any code is committed.

---

## TUI smoke test (tmux)

Use tmux to run the TUI and observe real terminal output. This is the only
reliable way to verify rendering, key handling, and filter behaviour.

```bash
# Always open a fresh window — never reuse a dirty one
tmux kill-window -t mex-test 2>/dev/null
tmux new-window -n mex-test
sleep 1
tmux send-keys -t mex-test "cd /home/roland/me/personal/mex && ./target/debug/mex 2>/dev/null" Enter
sleep 6   # wait for startup

# Capture what the terminal shows
tmux capture-pane -t mex-test -p | tail -10

# Send a key and re-capture
tmux send-keys -t mex-test "/" && sleep 0.3
tmux capture-pane -t mex-test -p | tail -4

# Clean up when done
tmux kill-window -t mex-test
```

### Rules

- Always kill the old window before creating a new one (`tmux kill-window -t mex-test 2>/dev/null`).
- Start mex with the pre-built binary (`./target/debug/mex`), not `cargo run`, to avoid keystrokes being swallowed by cargo's startup output.
- Use generous `sleep` delays after startup (≥ 5 s) and after key sends (≥ 0.2 s) before capturing.
- If the filter bar shows shell command text, the binary was not running yet when keys were sent — kill and retry with a fresh window.

### Build before testing

Always build before running the tmux test:

```bash
cd mex && cargo build 2>&1 | tail -3
```

If the build fails, fix the errors before attempting the TUI test.

---

## Chaos testing

After automated and smoke tests pass, the `mex-chaos-tester` skill stress-tests
the feature. See [.agents/agents/mex-chaos-tester/AGENT.md](../.agents/agents/mex-chaos-tester/AGENT.md).

Focus areas:
- Weird inputs (empty strings, unicode, extremely long text, special characters)
- Rapid key sequences and interrupted operations
- Large datasets (50 000+ files)
- Edge cases in filter combinations (#tag + @type + text)
- Concurrent operations (import while browsing, delete while filtering)
