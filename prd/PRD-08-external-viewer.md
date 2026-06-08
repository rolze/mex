# PRD-08 · External Viewer Launch

| Meta | Data |
|------|------|
| **Status** | `Draft` |
| **Derived from** | UC-12 (External Viewer) |

## Problem

Users need to open media files in their preferred external application (image viewer, video player, etc.) directly from the file list. The application must remain fully interactive while the external viewer runs, and failures must be communicated clearly without disrupting the workflow.

## User stories

- As a user, I want to open the file under my cursor in the system's default application with a single key press, so that I can view or edit it without leaving the application.
- As a user, I want the application to stay responsive while the external viewer is running, so that I can continue browsing my collection.
- As a user, I want a clear error message if the file cannot be opened, so that I understand what went wrong.

## Requirements

### Functional requirements

- **FR-1**: The application must provide a single key binding that opens the file under the cursor in the appropriate application based on its file extension (e.g., dedicated integrated viewers like `mpv` for video or `sem` for images, falling back to the OS default application for other types).
- **FR-2**: The external application must run in a separate process. The application must remain fully interactive — navigation, filtering, and all other functions must continue to work while the viewer is open.
- **FR-3**: When multiple files are selected, the launch action must always open the single file under the cursor, ignoring the multi-selection.
- **FR-4**: If the file does not exist on disk, the application must display a status error message instead of attempting to launch.
- **FR-5**: If the system launcher cannot be found or the launch fails for any other reason, the application must display a descriptive status error message.
- **FR-6**: The launch must work across all terminal emulators without relying on terminal-specific features.

### Non-functional requirements

- **NFR-1**: The launch action must not block the application's event loop. The user must perceive zero delay in the application's responsiveness.

## Acceptance criteria

- **AC-08-01**: Given a file list with a cursor on a valid file, when the user presses the external-viewer key binding, then the file opens in the system's default application and the application remains interactive.
- **AC-08-02**: Given multiple files are selected and the cursor is on a different file, when the user presses the external-viewer key binding, then only the cursor file is opened — the selection is ignored.
- **AC-08-03**: Given the cursor is on a file that no longer exists on disk, when the user presses the external-viewer key binding, then a status error message is displayed and no external process is launched.
- **AC-08-04**: Given the system has no default application configured for the file type (or the launcher is unavailable), when the user presses the external-viewer key binding, then a descriptive error message is displayed in the status area.
- **AC-08-05**: Given an external viewer has been launched, when the user navigates, filters, or performs any other action, then the application responds normally with no perceptible delay.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-12  | mex/spec/UC-12-open-external-viewer.md |


