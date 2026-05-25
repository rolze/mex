# Sem & Mex — Architecture Guidance

This document is the **source of truth for overarching design rules**, owned and
maintained by the `rust-architect` agent. It provides strict guardrails for the
`rust-developer` to ensure consistency, performance, and safety across all iterations.

Implementation-specific details (like exact module maps for a specific prototype
or exact protocol negotiations) do not belong here. They belong in an Architecture
Decision Log (`ADL.md`) located inside the specific implementation folder (e.g.,
`mex/ADL.md` or `mex_v1/ADL.md`).

---

## Architectural Foundation (The 4-Layer Model)

Based on the proven `mex_v1` architecture, all code must be strictly separated
into distinct layers to ensure extensibility, maintainability, and testability.

1. **`domain/`**
   - Pure data models (`MediaItem`, `Filter`, `Tag`, `Status`).
   - Maps exactly to the concepts in the PRD and Use Cases.
   - Zero dependencies on UI, database, or external I/O.
2. **`db/`**
   - Handles database interactions securely and efficiently (e.g., `rusqlite`).
   - Uses simple prepared statements inside transactions.
   - Caller-owned resources (accepts a borrowed `&Connection` or `&mut Connection`).
3. **`ui/`**
   - Fully decoupled TUI rendering system (e.g., `ratatui`).
   - Visual components (File List, Preview Pane, Filter Bar) are isolated modules
     that take only the required context.
4. **`services/`**
   - Holds command abstractions and background processes.
   - Decouples application routing and heavy lifting from the core event loop.
5. **`app.rs` & `main.rs`**
   - Drive the core event loop.
   - Standard threads, synchronous main loop, and clean state separation.

---

## Design Patterns

### Single-threaded UI, message-passing for everything else
The main thread owns all UI state and is the only thread that mutates `App`.
Background work runs on dedicated threads that communicate via `mpsc` channels.
**No shared mutable state** between UI and workers — every result flows back as
a typed message that the event loop drains and applies.

### Caller-owned resources
Library-style functions (notably in `db/`) accept the resources they need
as borrowed parameters. They never acquire them internally.
The caller decides resource scope and lifetime; the callee is pure logic over
borrowed inputs. This keeps hot paths fast and makes testing trivial.

### Monotonic IDs for async result staleness
Any work dispatched to a background thread carries a monotonic request ID.
When the response returns, the main thread compares the ID against the current
expected ID and drops stale responses.

### Explicit cache invalidation
Resetting cached state happens only on events that genuinely invalidate the context
(filter change, file deletion, navigation). UI transitions that do not change the
underlying content (toggling pane visibility) must not touch caches.

---

## Rust Non-Negotiables

These rules are load-bearing. Violating them re-introduces classes of bugs
the codebase has already paid to eliminate.

- **No hidden resource acquisition in library functions.** A function that
  takes `&str` and opens a `Connection` inside is forbidden.
- **`&T` for reads, `&mut T` only where the type's API forces it.** In
  practice this means `&Connection` for queries, `&mut Connection` only when
  starting a transaction.
- **Respect `Send` boundaries.** Non-`Send` resources (e.g., `rusqlite::Connection`)
  live in exactly one thread. Open once at thread start and reuse.
- **Cross-thread coordination uses mpsc, not `Arc<Mutex<_>>`.** Shared mutable
  state is the wrong primitive when one side is the UI event loop.
- **No `Connection::open` inside loops.** Open once outside the loop.
- **Errors propagate with `?`.** Library functions return `Result`; UI code
  surfaces errors as status messages. `.unwrap()` / `.expect()` are
  acceptable only in tests and fatal startup failures.
- **Tests use in-memory or temp-dir SQLite, never the user's DB.** Test
  helpers return live `Connection` objects so tests can both mutate and
  verify against the same handle.
- **Behavioural tests assert invariants, not implementation.** Tests verify
  state transitions — they do not assert on private fields.
