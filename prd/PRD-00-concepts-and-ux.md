# PRD-00 · Core Concepts and UX Guidelines

| Meta | Data |
|------|------|
| **Status** | `Approved` |

## Problem

Users need a consistent, responsive, and predictable terminal-based interface to manage large media collections. Without overarching UX guidelines, individual features might introduce confusing paradigms, inconsistent layouts, or blocking operations that degrade the overall experience. This foundational document establishes the core interaction model for all other features.

## User stories

- As a keyboard-centric power user, I want standard text-editor bindings and no mouse requirements, so that I can operate at maximum speed.
- As a user, I want the UI to remain responsive at all times, so that long-running tasks don't interrupt my workflow.
- As a user, I want consistent layout zones (list, preview, status, filter), so that my eyes always know where to look for specific information.
- As a user, I want non-destructive defaults and a universal escape hatch, so that I can explore and manage data safely without fear of irreversible mistakes.

## Requirements

### Functional requirements

- **FR-1**: The application operates entirely via keyboard input without requiring a mouse.
- **FR-2**: The interface adheres to a consistent structural layout: a primary list view on the left, a Multipurpose Context-Aware Panel on the right (used for on-demand details/previews AND background task status/interaction), an input box for filters/commands at the bottom left, and a dedicated status box at the bottom right.
- **FR-3**: The application operates in three primary interaction modes:
  - **Normal Mode**: Navigation, quick media actions (play, view), and state toggles.
  - **Filter Mode**: Live-updating search and narrowing of the item list.
  - **Command Mode**: Execution of complex actions with arguments.
- **FR-4**: The application supports multi-selection, and all actions that target files must fallback to the currently highlighted item if the selection is empty.
- **FR-5**: Operations that manipulate data (trashing, modifying) must be reversible or safe by default. Hard deletes must require explicit confirmation.
- **FR-6**: The application uses `<Tab>` to cycle keyboard focus between the active UI panes (List Pane → Input/Filter Box → optionally the Multipurpose Context-Aware Panel if a background task requires interaction). The Right Pane cannot receive focus if it is only displaying a file preview.
- **FR-7**: The `Esc` key acts as a contextual escape hatch for the currently focused pane. Rather than a global hierarchy, `Esc` clears states or aborts actions specific to the active pane (e.g., aborts a background task if focused on the context panel; clears filter/selection if focused on the list).
- **FR-8**: The currently focused pane must be visually distinct, primarily indicated by a change in its border color.
- **FR-9**: The UI provides explicit context and discoverability via inline auto-suggestions, tab-completion (when in input modes), and dedicated status reporting areas.

### User Input Sanitization Policy

- **FR-10**: The system must proactively sanitize user input in real-time using a strict allow-list and auto-conversion approach. Invalid keystrokes must be rejected with a brief status hint as they are typed.
- **FR-11**: **Metadata Tokens**: When typing tags, types, or slugs, the system allows alphanumerics (`a-z`, `0-9`), hyphens (`-`), underscores (`_`), and single spaces (` `). Spaces are forbidden in slugs (auto-converted to hyphens).
- **FR-12**: **Plain Text Filters**: When typing a plain text filter (which matches filenames), the system allows `a-z`, `0-9`, `-`, `.` (for extensions), and `*` (for wildcard matching).
- **FR-13**: **Auto-Conversion**: Across all inputs, the system must automatically:
  - Downcase uppercase letters.
  - Convert spaces (` `) and underscores (`_`) into hyphens (`-`) for text filters and slugs.
  - Transliterate diacritics/umlauts (e.g., `ä` -> `ae`, `ß` -> `ss`).
- **FR-14**: **Control Prefixes**: Prefixes such as `#` (tags), `@` (types), and `:` (commands) are only valid as mode triggers. They are rejected if typed mid-string or duplicated.
- **FR-15**: **Separators**: Consecutive separators (e.g., `--` or `  `) and leading/trailing separators must be proactively rejected.
- **FR-16**: **Forbidden Characters**: Any character not explicitly allowed (e.g., `/`, `\`, `!`, `?`, `<`, `>`, `"`, `'`, `%`, `&`, `|`, `(`, `)`) must be rejected with a status hint.

### Non-functional requirements

- **NFR-1**: All user interface navigation and filtering interactions must be perceptually instant and non-blocking.
- **NFR-2**: Long-running operations must execute asynchronously. Only one background task may run at a time to prevent interference. Tasks must take over the Multipurpose Context-Aware Panel on the right to display progress and allow interaction. While a background task is running, standard file previews are unavailable.
- **NFR-3**: Background tasks must ensure data store and file system consistency even if aborted mid-execution.

## Acceptance criteria

- **AC-1**: Given any state of the application, when a long-running task is triggered, then the UI remains responsive to further keyboard navigation and interactions.
- **AC-2**: Given a long-running task is active and its pane is focused, when the user presses `Esc`, then the task safely aborts without leaving the data store or file system in a corrupted state.
- **AC-3**: Given multiple panes in the UI, when the user presses `<Tab>`, then focus visually and logically shifts between the panes, determining where contextual keys (like `Esc`) apply.
- **AC-4**: Given an empty selection, when an action is executed, then it targets the item currently under the cursor.
- **AC-5**: Given the application is running, the layout explicitly reserves distinct space for the input/filter box and the status box at the bottom of the interface.
- **AC-6**: Given the user attempts a destructive action, then the system requests explicit confirmation before proceeding.

## Success metrics

- UI response latency is consistently under 16ms.
- Zero instances of data corruption due to aborted background operations.
- Keyboard-only navigation completely covers all application capabilities.

## Constraints

- No reliance on mouse input or graphical windowing system paradigms.
- The application operates entirely within a terminal emulator environment.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-00  | mex/spec/UC-00-concepts-and-ux.md |
