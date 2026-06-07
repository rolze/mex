# PRD-00 · Core Concepts and UX Guidelines

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
- **FR-2**: The interface adheres to a consistent structural layout: a primary list view on the left, an on-demand detail/preview pane on the right, an input box for filters/commands at the bottom left, and a dedicated status box at the bottom right.
- **FR-3**: The application operates in three primary interaction modes:
  - **Normal Mode**: Navigation, quick media actions (play, view), and state toggles.
  - **Filter Mode**: Live-updating search and narrowing of the item list.
  - **Command Mode**: Execution of complex actions with arguments.
- **FR-4**: The application supports multi-selection, and all actions that target files must fallback to the currently highlighted item if the selection is empty.
- **FR-5**: Operations that manipulate data (trashing, modifying) must be reversible or safe by default. Hard deletes must require explicit confirmation.
- **FR-6**: The application uses the `Esc` key as a universal, hierarchical escape hatch that sequentially steps back UI states (e.g., aborts background tasks → clears filter → closes preview → clears selection).
- **FR-7**: The UI provides explicit context and discoverability via inline auto-suggestions, tab-completion, and dedicated status reporting areas.

### Non-functional requirements

- **NFR-1**: All user interface navigation and filtering interactions must be perceptually instant and non-blocking.
- **NFR-2**: Long-running operations must execute asynchronously in the background while providing progress updates (e.g., overlays or status box updates) to the user interface.
- **NFR-3**: Background tasks must ensure data store and file system consistency even if aborted mid-execution.

## Acceptance criteria

- **AC-1**: Given any state of the application, when a long-running task is triggered, then the UI remains responsive to further keyboard navigation and interactions.
- **AC-2**: Given a long-running task is active, when the user presses `Esc`, then the task safely aborts without leaving the data store or file system in a corrupted state.
- **AC-3**: Given multiple active UI states (e.g., selection made, preview open, filter active), when the user repeatedly presses `Esc`, then the states are sequentially cleared one by one.
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

## Open questions

- Are there specific timeout thresholds required before an asynchronous task is considered "failed" rather than "aborted"?
- What is the exact sequence priority for the `Esc` key step-back if multiple overlay panels are open simultaneously?
