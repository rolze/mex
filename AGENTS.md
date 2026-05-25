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
| Architecture | `doc/ARCHITECTURE.md` | Core structural guardrails, layered model, Rust non-negotiables |
| Dev setup | `doc/DEV.md` | Build, release pipeline, prerequisites |
| Database | `doc/DATABASE.md` | Schema, column rationale, performance |
| Filename spec | `doc/REGEXP.md` | Strict filename convention and regex |
| Testing | `doc/TESTING.md` | Automated tests, TUI smoke test, chaos testing |
| Install | `INSTALL.md` | End-user install instructions |
| Use cases | `<variant>/spec/UC-XX.md` | Human-written, implementation-flavoured (see `<variant>/spec/README.md`) |
| PRDs | `prd/PRD-XX-*.md` | Technology-agnostic requirements (see `prd/README.md`) |

**Read `doc/ARCHITECTURE.md` before writing any code.** It contains load-bearing design rules.

**Read the relevant `prd/PRD-XX-*.md` before planning any feature.** PRDs define
what the product must do; `doc/ARCHITECTURE.md` defines the structural guardrails for how to build it. Prototype-specific implementation details are kept in local `ADL.md` files within the implementation folders.

---

## AIDLC — AI Development Life Cycle

Six phases. Agents run autonomously through all phases.
Human is engaged only to resolve contradictions, unclear requirements, or for final review.

```
 Spec ──▶ Plan ──▶ Implement ──▶ Review ──▶ Test ──▶ Ship
 auto     auto     auto          auto       auto     human
```

The `requirements-engineer` is active across all phases — see Agent Roles below.

### Phase 1 — Spec

1. Read the relevant `<variant>/spec/UC-XX.md` documents.
2. `requirements-engineer` converts or updates `prd/PRD-XX-*.md` from the UC docs.
   PRDs must be technology-agnostic — no Rust, Ratatui, SQLite, or GTK4 references.
   Each PRD has testable acceptance criteria in Given/When/Then format.
3. If a new feature has no UC, draft a UC in `<variant>/spec/` first, then derive the PRD.
4. Identify gaps: acceptance criteria without matching requirements, or requirements
   without acceptance criteria. Flag as open questions.

**Output**: Updated PRDs with acceptance criteria. List of open questions.

### Phase 2 — Plan

1. Produce an implementation plan with:
   - Files to create / modify / delete.
   - Database schema changes (if any).
   - Architectural decisions validated against `doc/ARCHITECTURE.md` patterns and local `ADL.md` rules.
2. `requirements-engineer` validates: does the plan cover **every** acceptance
   criterion in the relevant PRDs? Flag missed requirements.
3. Flag anything ambiguous or contradictory — do not guess.
4. If conflicting requirements are found, `requirements-engineer` resolves by
   consulting the PRD → UC → human (in that order).

**Output**: Implementation plan artifact. Proceed to Phase 3 unless blocked.

### Phase 3 — Implement

1. Write code following the `rust-developer` skill.
2. Update the relevant `<variant>/spec/UC-XX.md` in the same commit — see UC sync rules below.
3. Keep commits small, focused, and atomic.
4. Commit message format: `<area>: <what changed>` — e.g. `import: deduplicate by partial hash`, `ui: add tag autocomplete to command bar`.
5. When ambiguities arise, escalate to `requirements-engineer` — not the human.
   The requirements-engineer resolves by updating the PRD.

**Output**: Working code with updated UC docs.

### Phase 4 — Review

1. `requirements-engineer` verifies: does the implementation satisfy **all** PRD
   acceptance criteria? Reports coverage: `AC-N: covered / not covered / partially covered`.
2. `rust-architect` reviews: async correctness, caching, allocation, `Send` boundaries.
3. `database-expert` reviews schema changes (if any): indexes, triggers, migrations.
4. `ux-designer` reviews UI changes (if any): colour, layout, interaction quality.
5. Each reviewer outputs a verdict: `Approved` / `Concerns Raised` / `Major Issues`.
6. `Major Issues` blocks Phase 5. `Concerns Raised` must be addressed or explicitly deferred.

**Output**: Review verdicts with requirements coverage. Rework if needed, then proceed.

### Phase 5 — Test

1. Run automated tests: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
2. Run TUI smoke test via tmux — see `doc/TESTING.md`.
3. `mex-chaos-tester` stress-tests edge cases.
4. `rust-quality-checker` runs the full quality pipeline.
5. `requirements-engineer` confirms: do test cases map to PRD acceptance criteria?
   Flags untested requirements.
6. All tests must pass. Fix failures and re-run.

**Output**: All-green test results with requirements traceability.

### Phase 6 — Ship

1. Summarize changes in a walkthrough artifact.
2. Present the final diff to the human for review.
3. Human approves, requests changes, or rejects.
4. On approval: commit to `main`. Tag a release if appropriate (`git tag vX.Y.Z`).

---

## Agent Roles

Seven specialized agents collaborate through the AIDLC. Each has a detailed
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

### requirements-engineer

Converts human-written use cases (`<variant>/spec/UC-XX.md`) into technology-agnostic
PRD documents (`prd/PRD-XX-*.md`). Owns the PRDs. Defines acceptance criteria
and success metrics. Resolves ambiguities and conflicts between agents.
The requirements-engineer ensures implementations satisfy product intent
regardless of the technology stack used.

**Invoked in**: Phase 1 (primary), Phase 2 (validate plan), Phase 3 (on-call),
Phase 4 (verify acceptance criteria), Phase 5 (confirm test coverage).
**Skill**: `.agents/skills/requirements-engineer/SKILL.md`

### rust-developer

Implements features and fixes. Writes clean, idiomatic, testable Rust code.

**Invoked in**: Phase 3 (Implement).
**Skill**: `.agents/skills/rust-developer/SKILL.md`

### rust-architect

Reviews code for async correctness, runtime efficiency, caching design, and
adherence to `doc/ARCHITECTURE.md` patterns. Owns the architecture guidance.
Skeptical by default — does not rubber-stamp.

**Invoked in**: Phase 4 (Review).
**Skill**: `.agents/skills/rust-architect/SKILL.md`

### database-expert

Reviews and designs SQLite schema, queries, indexes, and migrations.
Challenges every table and index to earn its keep. Owns and authoritatively
maintains the `doc/DATABASE.md` schema guidance.

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

Every UC document (`<variant>/spec/UC-XX.md`) is the human's voice for its feature.

**Rule: every implementation change must be reflected in the corresponding UC document in the same commit.**

- If you add, remove, or change a behaviour, update the UC doc to match.
- UC docs must be brief and concise — describe *what is implemented*, not aspirations.
- Remove outdated details immediately; do not leave stale text.
- If a change spans multiple UCs, update all affected docs.

### PRD document ownership

Every PRD document (`prd/PRD-XX-*.md`) is owned by the `requirements-engineer`.

**Rule: only the requirements-engineer creates or modifies PRDs.**

- PRDs are technology-agnostic. No Rust, Ratatui, SQLite, or GTK4 references.
- Every PRD has acceptance criteria in Given/When/Then format.
- When a UC changes, the requirements-engineer reviews the corresponding PRD.
- When agents disagree about requirements, the PRD is the arbiter.
- If the PRD doesn't answer the question, escalate to the human.

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
- See `doc/ARCHITECTURE.md` § "Rust Non-Negotiables" for the full list.

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

**Phase 1 — Spec**: `requirements-engineer` reads UC-02 (browse), UC-04 (selecting),
UC-14 (caption). The rename feature is closest to UC-14 but broader. Drafts UC-17.
Then distils PRD-17-rename with acceptance criteria:
- AC-1: Given a selected file, when the user executes rename with a valid name, then the file is renamed and the list updates instantly.
- AC-2: Given a name that conflicts with an existing file, when rename is executed, then the operation fails with a clear error — no data loss.
- AC-3: Given a file with tags, when renamed, then all tag associations are preserved.

**Phase 2 — Plan**: Rename touches `app.rs` (command handling), `db.rs` (path_stem update),
and filesystem (atomic rename + mtime preservation). Validate against `doc/ARCHITECTURE.md`:
caller-owned resources (pass `&mut Connection`), single-threaded UI (rename is synchronous —
fast enough for one file). Plan: add `:rename` command parser in `app.rs`,
`rename_file()` in `db.rs`, filesystem rename in `app.rs` command handler.

**Phase 3 — Implement**: `rust-developer` writes the code. During implementation,
asks: "should rename preserve the file's modification timestamp?" →
`requirements-engineer` checks PRD-17, finds no requirement, adds NFR-1:
"Rename must preserve the file's original modification timestamp."

**Phase 4 — Review**: `requirements-engineer` verifies AC-1 ✅, AC-2 ✅, AC-3 ✅,
NFR-1 ✅. `rust-architect` checks the rename is atomic. `ux-designer` checks the
status message colour and wording after rename.

**Phase 5 — Test**: `cargo test` passes. `requirements-engineer` confirms test cases
map to AC-1, AC-2, AC-3, NFR-1. `mex-chaos-tester` tries: rename to existing name,
rename with special characters, rename during active filter, rename a file that was
just deleted.

**Phase 6 — Ship**: Present diff to human. Human approves. Commit to main.
