# Agent Instructions — Sem & Mex

This file is the entry point for any AI agent working on this codebase.
Read it fully before making any change.

## Project

`mex` is a Rust + Ratatui terminal media browser and importer backed by SQLite.
`sem` is a companion GTK4 image viewer launched by mex.
Together they are **Sem & Mex**.

| Crate | Path | Purpose |
|-------|------|---------|
| mex | `mex/` | TUI: browse, filter, tag, import, preview |
| sem | `sem/` | GTK4 image viewer: single image + grid |

## Quick reference

```bash
cd mex && cargo build                          # build
cd mex && cargo test                           # test
cd mex && cargo clippy -- -D warnings          # lint (zero warnings)
cd mex && cargo fmt --check                    # format check
./target/debug/mex 2>/dev/null                 # run
```

All four checks must pass before committing.

## Documentation

| Document | Path | Content |
|----------|------|---------|
| README | `README.md` | What Sem & Mex does, features, config |
| Architecture | `doc/IMPL.md` | Module map, patterns, Rust non-negotiables |
| Dev setup | `doc/DEV.md` | Build, release pipeline, prerequisites |
| Database | `doc/DATABASE.md` | Schema, column rationale, performance |
| Filename spec | `doc/REGEXP.md` | Strict filename convention and regex |
| Testing | `doc/TESTING.md` | Automated tests, TUI smoke test, chaos testing |
| Install | `INSTALL.md` | End-user install instructions |
| Use cases | `spec/UC-XX.md` | Source of truth per feature (see `spec/README.md`) |

**Read `doc/IMPL.md` before writing any code.** It contains load-bearing design rules.

---

## AIDLC — AI Development Life Cycle

Six phases. Agents run autonomously through all phases.
Human is engaged only to resolve contradictions, unclear requirements, or for final review.

```
 Spec ──▶ Plan ──▶ Implement ──▶ Review ──▶ Test ──▶ Ship
 auto     auto     auto          auto       auto     human
```

### Phase 1 — Spec

1. Read the relevant `spec/UC-XX.md` documents.
2. If the feature is covered by an existing UC, note what must change.
3. If a new UC is needed, draft it following the format in `spec/`.
4. Identify affected modules from `doc/IMPL.md` module map.
5. If the feature touches the database, read `doc/DATABASE.md`.

**Output**: List of affected UC docs, modules, and open questions.

### Phase 2 — Plan

1. Produce an implementation plan with:
   - Files to create / modify / delete.
   - Database schema changes (if any).
   - Architectural decisions validated against `doc/IMPL.md` patterns.
2. Flag anything ambiguous or contradictory — do not guess.
3. If conflicting requirements are found, stop and request human clarification.

**Output**: Implementation plan artifact. Proceed to Phase 3 unless blocked.

### Phase 3 — Implement

1. Write code following the `rust-developer` skill.
2. Update the relevant `spec/UC-XX.md` in the same commit — see UC sync rules below.
3. Keep commits small, focused, and atomic.
4. Commit message format: `<area>: <what changed>` — e.g. `import: deduplicate by partial hash`, `ui: add tag autocomplete to command bar`.

**Output**: Working code with updated UC docs.

### Phase 4 — Review

1. `rust-architect` reviews: async correctness, caching, allocation, `Send` boundaries.
2. `database-expert` reviews schema changes (if any): indexes, triggers, migrations.
3. `ux-designer` reviews UI changes (if any): colour, layout, interaction quality.
4. Each reviewer outputs a verdict: `Approved` / `Concerns Raised` / `Major Issues`.
5. `Major Issues` blocks Phase 5. `Concerns Raised` must be addressed or explicitly deferred.

**Output**: Review verdicts. Rework if needed, then proceed.

### Phase 5 — Test

1. Run automated tests: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
2. Run TUI smoke test via tmux — see `doc/TESTING.md`.
3. `mex-chaos-tester` stress-tests edge cases.
4. `rust-quality-checker` runs the full quality pipeline.
5. All tests must pass. Fix failures and re-run.

**Output**: All-green test results.

### Phase 6 — Ship

1. Summarize changes in a walkthrough artifact.
2. Present the final diff to the human for review.
3. Human approves, requests changes, or rejects.
4. On approval: commit to `main`. Tag a release if appropriate (`git tag vX.Y.Z`).

---

## Agent Roles

Six specialized agents collaborate through the AIDLC. Each has a detailed
skill definition in `.agents/skills/`.

### Orchestrator

The top-level agent. Drives the AIDLC phases, delegates to specialists,
resolves inter-agent disagreements, and decides when to engage the human.

**Responsibilities**:
- Sequence the AIDLC phases.
- Invoke the right specialist for each phase.
- Aggregate review verdicts and decide proceed/rework/escalate.
- Maintain the task artifact and track progress.
- Engage the human only for: contradictions, unclear requirements, final review.

No separate skill file — the orchestrator is the agent reading this document.

### rust-developer

Implements features and fixes. Writes clean, idiomatic, testable Rust code.

**Invoked in**: Phase 3 (Implement).
**Skill**: `.agents/skills/rust-developer/SKILL.md`

### rust-architect

Reviews code for async correctness, runtime efficiency, caching design, and
adherence to `doc/IMPL.md` patterns. Skeptical by default — does not rubber-stamp.

**Invoked in**: Phase 4 (Review).
**Skill**: `.agents/skills/rust-architect/SKILL.md`

### database-expert

Reviews and designs SQLite schema, queries, indexes, and migrations.
Challenges every table and index to earn its keep.

**Invoked in**: Phase 2 (Plan, if schema changes), Phase 4 (Review).
**Skill**: `.agents/skills/database-expert/SKILL.md`

### ux-designer

Guards visual polish, colour harmony, layout balance, and interaction feel
in the Ratatui TUI. Pushes for playfulness and delight over clinical defaults.

**Invoked in**: Phase 4 (Review, if UI changes).
**Skill**: `.agents/skills/ux-designer/SKILL.md`

### mex-chaos-tester

Playful chaos monkey. Tests aggressively, explores edge cases, deliberately
deviates from the happy path to expose bugs and performance issues.

**Invoked in**: Phase 5 (Test).
**Skill**: `.agents/skills/mex-chaos-tester/SKILL.md`

### rust-quality-checker

Runs the full quality pipeline: rustfmt, clippy, cargo check, cargo test,
security audit, dependency analysis.

**Invoked in**: Phase 5 (Test).
**Skill**: `.agents/skills/rust-quality-checker/SKILL.md`

---

## Rules

### UC document sync

Every UC document (`spec/UC-XX.md`) is the source of truth for its feature.

**Rule: every implementation change must be reflected in the corresponding UC document in the same commit.**

- If you add, remove, or change a behaviour, update the UC doc to match.
- UC docs must be brief and concise — describe *what is implemented*, not aspirations.
- Remove outdated details immediately; do not leave stale text.
- If a change spans multiple UCs, update all affected docs.

### Product name: "Sem & Mex"

The product pair is always written **"Sem & Mex"** — Sem first, ampersand separator, capital S and M.

In all documentation, comments, headings, and commit messages, write `Sem & Mex` — never `mex and sem`, `mex & sem`, `mex + sem`, or any other reversed or alternative form.

This applies to prose that refers to the two tools together as a product pair. It does **not** apply to:
- Sentences describing the runtime relationship (e.g. "mex spawns sem") — these describe causality, not the product name.
- Operational code where argument/file order is functionally irrelevant (e.g. `chmod +x mex sem`).

### Code style

- `rustfmt` — always. No exceptions.
- `clippy` — zero warnings (`-D warnings`).
- No `.unwrap()` / `.expect()` in library code. Acceptable only in tests and fatal startup.
- Errors propagate with `?`. Library functions return `Result`.
- `&T` for reads, `&mut T` only where the type's API forces it.
- Cross-thread coordination uses mpsc, not `Arc<Mutex<_>>`.
- See `doc/IMPL.md` § "Rust non-negotiables" for the full list.

### Commit messages

Format: `<area>: <brief description>`

```
import: deduplicate by partial hash instead of full SHA-256
ui: add alternating row shading to file list
db: add idx_media_path_stem index for collision queries
fix: handle empty filter string without panic
```

Keep messages brief and consistent — enough context to extract meaningful changelogs.
No conventional-commits prefixes (`feat:`, `fix:`, `chore:`). Use the module or area name.

### Testing

See `doc/TESTING.md` for full instructions. Summary:

```bash
cargo test                           # all tests pass
cargo clippy -- -D warnings          # zero warnings
cargo fmt --check                    # formatting clean
```

TUI smoke testing uses tmux — see `doc/TESTING.md` § "TUI smoke test".

---

## Workflow Example

> Feature request: "Add `:rename` command to rename the selected file."

**Phase 1 — Spec**: Read UC-02 (browse), UC-04 (selecting), UC-14 (caption).
The rename feature is closest to UC-14 (caption editing) but broader.
Decide: extend UC-14 or create UC-17. Draft UC-17.

**Phase 2 — Plan**: Rename touches `app.rs` (command handling), `db.rs` (path_stem update),
and filesystem (atomic rename + mtime preservation). Validate against IMPL.md:
caller-owned resources (pass `&mut Connection`), single-threaded UI (rename is synchronous —
fast enough for one file). Plan: add `:rename` command parser in `app.rs`,
`rename_file()` in `db.rs`, filesystem rename in `app.rs` command handler.

**Phase 3 — Implement**: `rust-developer` writes the code, adds tests for
`rename_file()` with in-memory SQLite, updates UC-17 with the implemented behaviour.

**Phase 4 — Review**: `rust-architect` checks the rename is atomic (rename then
update DB in a transaction — not the reverse). `ux-designer` checks the status
message colour and wording after rename.

**Phase 5 — Test**: `cargo test` passes. Tmux smoke test: navigate to a file,
type `:rename new-name`, verify the file list updates. `mex-chaos-tester` tries:
rename to existing name, rename with special characters, rename during active filter,
rename a file that was just deleted.

**Phase 6 — Ship**: Present diff to human. Human approves. Commit to main.
