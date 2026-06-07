# PRD-02 · Browse Media Files

## Problem

Users need a way to quickly navigate, filter, and inspect their indexed media collection. The interface must be highly responsive to keyboard input and provide immediate feedback on file state, filtering, and selection.

## User stories

- As a user, I want to see my media files in a structured list with folder, filename, and tags, so that I can quickly identify them.
- As a user, I want to navigate the list using keyboard controls, so that I can browse without leaving the home row.
- As a user, I want to instantly filter the list by text or tags, so that I can find specific files in large collections.
- As a user, I want visual feedback on missing or broken files, so that I can manage my collection's integrity.
- As a user, I want to be able to mark files as trashed or keep them, and play media via external players using quick keyboard controls.

## Requirements

### Functional requirements

- **FR-1**: The application displays a split-pane interface: a file list and bottom status area on the left, and an image preview on the right.
- **FR-2**: The file list displays items with fixed-width columns: folder, filename, and tags. Filenames are truncated at the front if they exceed column width, preserving the tail. Tags are truncated at the end. Files without tags display a `—` placeholder.
- **FR-3**: Files are sorted by path, grouping collision siblings immediately after their base file.
- **FR-4**: The application supports keyboard navigation for cursor movement (up/down row, half page, full page, jump group) and toggling selection (Space) / preview (Enter). Full page navigation dynamically jumps by the current terminal window's visible list height.
- **FR-5**: The application provides a filter mode that accepts text or tag patterns and live-updates the file list. The user must explicitly press `/`, `#`, or `@` to enter filter mode, preventing accidental text input during normal navigation.
- **FR-6**: The filter supports wildcard (`*`) matching for text segments.
- **FR-7**: The application provides a command mode for executing textual commands.
- **FR-8**: The application displays a bottom layout split into two distinct bordered boxes: a Filter box that shows shortcut hints, trash counts, and the active filter state, and a separate Status box that shows live feedback and external player status.
- **FR-9**: The application highlights matched text segments in the filename column during active filtering. Unselected filenames exhibit semantic colorization to distinguish structural parts (year, dates, topic, subject, etc.). Selected items are highlighted distinctly (e.g., cyan background with bold black text).
- **FR-10**: The application indicates when a physical file is missing from the data store with a distinct marker and styling. The missing status is checked lazily when previewing the file.
- **FR-11**: The application maps standard keyboard actions for file management and media: `Delete` to trash, `Insert` to keep, and `j`/`k`/`p`/`s` for external media player (mpv) controls.

### Non-functional requirements

- **NFR-1**: Navigation and scrolling must be perceptually instant (no visible lag on lists up to 50,000 entries).
- **NFR-2**: Wildcard filtering must be perceptually instant on lists up to 50,000 entries.
- **NFR-3**: The application must auto-scroll to keep the selected cursor row visible during navigation.

## Acceptance criteria

- **AC-1**: Given an indexed collection, when the user opens the application, then the file list is displayed with compact folder, filename, and tag columns. Files without tags display a `—` placeholder. Unselected filenames exhibit semantic colorization.
- **AC-2**: Given a set of files with collisions (e.g., base.ext, base-2.ext), when the list is sorted, then base.ext appears immediately before base-2.ext regardless of standard byte-order sorting.
- **AC-3**: Given a list of files, when the user navigates using arrow keys or page keys, then the cursor moves accordingly, the list auto-scrolls to keep the cursor visible, and PageUp/PageDown jumps exactly by the visible list height.
- **AC-4**: Given an active filter mode, when the user types a text pattern, then the list updates instantly to show only matching files, and matching segments in the filename column are highlighted.
- **AC-5**: Given an active filter mode, when the user uses a wildcard `*`, then files matching the wildcard pattern are displayed, and only the matched literal segments are highlighted.
- **AC-6**: Given the application is running, when the user copies the path of the current file, then the full absolute path is placed on the clipboard, and a status message is shown in the separate Status box.
- **AC-7**: Given a file that no longer exists on disk, when the user triggers a preview, then the application lazily detects it is missing, updates the state, and displays the missing marker/styling in the list.
- **AC-8**: Given the filter or command mode is active, then the border of the Filter box is highlighted distinctly.
- **AC-9**: Given the application is running, the bottom of the interface displays two side-by-side bordered boxes: a Filter box with shortcut hints and a Status box with live feedback.
- **AC-10**: Given an item is selected, when the user presses `Delete`, then the item is marked as trashed, and when `Insert` is pressed, the item is restored.

## Success metrics

- Scrolling and filtering latency under 16ms (60 FPS) for 50,000 items.
- All keyboard shortcuts perform their expected actions immediately.

## Constraints

- Filename truncation must prioritize the file tail (extension and trailing characters).
- Status messages must not overwrite the active filter text in the UI.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-02  | mex/spec/UC-02-browse-media.md |
| UC-03  | mex/spec/UC-03-image-preview.md (referenced) |
| UC-04  | mex/spec/UC-04-selection.md (referenced) |
| UC-00  | mex/spec/UC-00-concepts-and-ux.md |

## Open questions

- Are there specific color tokens required for the missing file marker, or should it rely on the UX designer's theme?
- Is there a defined maximum length for the status messages in the new Status box?
