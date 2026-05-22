## UC-00 · Key concepts and UX guidelines

This is the foundational guideline for all mex usecases. Keep this brief, concise, and refer to it when designing new features or agents.

## Key Principles & UX Decisions

* **Terminal-First & Keyboard-Driven:** No mouse required. Intuitive for users of `vi`, `tmux`, and shells (e.g., `hjkl` concepts, `/` for filter, `:` for commands).
* **Speed & Non-Blocking UI:** Navigation and filtering must be instant. Long-running tasks (imports, renaming) must run in background threads, unblocking the UI, providing progress overlays (spinners, bars), and supporting safe aborts via `Esc`.
* **Context & Discoverability:** The user must always know the system state and available actions.
  * Heavy use of inline auto-suggestions (dimmed text) and tab-completion for tags, commands, and paths.
  * Explicit status reporting: Feedback (success, errors, guardrail warnings) is printed to the dedicated Status box or right-aligned in the input bar.
* **General Layout (Split-Pane):**
  * **Main (Left):** File list with fixed-width columns.
  * **Preview (Right):** Detailed metadata and image/video preview on demand.
  * **Input Bar (Bottom Left):** Unified bar for filters (`/`) and commands (`:`). Visually distinct when active (e.g., yellow border).
  * **Status Box (Bottom Right):** Live feedback (e.g., mpv state, background task status).
* **Modes:**
  * **Normal Mode:** Navigation (`↑/↓`, `Home/End`), quick actions (`p`, `s`, `Space`, `Del`, `Ins`), and jumping.
  * **Filter Mode (`/`):** Text matching, tag matching (`#`), and tag-type matching (`@`).
  * **Command Mode (`:`):** Execution of complex actions with arguments (`:import`, `:fix-date`).
* **Selection & Execution Model:**
  * Multi-selection via `Space` (toggle) and `Shift`+navigation (range sweep).
  * Actions gracefully fallback to the cursor item if the selection is empty.
* **Safety & Idempotency:**
  * Never destruct data blindly. Operations like fixing extensions or trashing files (`Delete`) should be reversible. Hard deletes (`:empty-trash`) require explicit confirmation and operate in guarded batches.
  * Background operations must leave the database and filesystem in a consistent state if aborted.
* **Consistent Escape Hatch:**
  * `Esc` provides a universal way to step back: clears selection -> closes preview -> clears filter -> aborts background tasks.
