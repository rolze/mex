# PRD-06 · File Selection & Bulk Operations

| Meta | Data |
|------|------|
| **Status** | `Draft` |
| **Derived from** | UC-04 (Selection) |

---

## Problem

Users managing large media collections need to select one or many files quickly and precisely before applying bulk operations (tagging, trashing, moving, etc.). A single-item cursor is insufficient — users need toggle selection, contiguous range sweeps, group-aware jumps, and a select-all shortcut. Without these, bulk workflows become tedious and error-prone.

## User stories

- As a user, I want to toggle individual files in and out of a selection, so that I can cherry-pick exactly which files to act on.
- As a user, I want to sweep a contiguous range of files into the selection using a shift-modified directional key, so that I can select large runs without repetitive toggling.
- As a user, I want to jump to the start or end of the current logical group, so that I can navigate and select within meaningful boundaries (e.g., all photos from the same day or slug).
- As a user, I want to select or deselect everything in the current result set with a single shortcut, so that I can quickly start a fresh selection or act on the whole set.
- As a user, I want clear visual feedback showing which files are selected and how many, so that I never lose track of my selection before executing an operation.

## Requirements

### 2.1 Individual Toggle

| ID | Requirement |
|----|-------------|
| R-06-01 | The user must be able to toggle the file under the cursor in or out of the selection with a single key press. |
| R-06-02 | Toggling an individual file must clear any active range-sweep anchor, so that subsequent range operations start fresh. |

### 2.2 Range Sweep Selection

| ID | Requirement |
|----|-------------|
| R-06-03 | The user must be able to extend the selection by holding a modifier and pressing a directional key (up or down) to sweep a contiguous range. |
| R-06-04 | On the first press of a range sweep (or when the sweep direction reverses), both the originating file and the file landed on must be toggled. |
| R-06-05 | On subsequent presses in the same direction, only the newly landed file must be toggled (the originating file is not re-toggled). |
| R-06-06 | The range sweep must produce a clean, uninterrupted selection when the modifier is held continuously in one direction. |

### 2.3 Group-Aware Cursor Jumps

| ID | Requirement |
|----|-------------|
| R-06-07 | The application must derive logical groups from its filename convention: files sharing a common slug are grouped by the slug portion, and day-based files are grouped by their date. |
| R-06-08 | A "jump to group start" action must move the cursor to the first file in the current group. If the cursor is already at the group's first file, it must jump to the first file of the previous group. |
| R-06-09 | A "jump to next group" action must move the cursor to the first file of the next group. If the cursor is already in the last group, the action must be a no-op. |

### 2.4 Group-Aware Range Selection

| ID | Requirement |
|----|-------------|
| R-06-10 | The user must be able to range-select from the cursor to the start of the current group using a modifier combined with the group-start jump. If the cursor is already at the group start, the range must extend to the start of the previous group. |
| R-06-11 | The user must be able to range-select from the cursor to the end of the current group using a modifier combined with the group-end jump. After the selection, the cursor must advance to the first file of the next group. |
| R-06-12 | Group-aware range selections must use all-or-nothing toggle logic: if any file in the target range is unselected, all files in the range become selected; if every file in the range is already selected, all files in the range become deselected. |

### 2.5 Select All / Deselect All

| ID | Requirement |
|----|-------------|
| R-06-13 | A "select all" action must select every file in the current result set (i.e., the filtered list, not the entire collection). |

### 2.6 Persistence & Scope

| ID | Requirement |
|----|-------------|
| R-06-14 | The selection state must persist across text filter changes, allowing users to build a selection across multiple searches. |
| R-06-15 | Bulk operations must apply to all currently selected files, including those temporarily hidden by an active filter. |
| R-06-16 | The visual marker denoting selected rows must be configurable via the application theme. |
| R-06-14 | If every file in the current result set is already selected, the same action must deselect all. |

### 2.6 Escape Hierarchy

| ID | Requirement |
|----|-------------|
| R-06-15 | The universal escape action must clear the current selection as its first step. Subsequent presses handle other UI states (preview, filter) as defined by the application's escape hierarchy. |

### 2.7 Cursor Fallback

| ID | Requirement |
|----|-------------|
| R-06-16 | When the selection is empty, any operation that would normally act on the selected files must fall back to the single file under the cursor. |

### 2.8 Post-Operation Behaviour

| ID | Requirement |
|----|-------------|
| R-06-17 | After a bulk operation completes successfully, the selection must be automatically cleared. |

### 2.9 Visual Feedback

| ID | Requirement |
|----|-------------|
| R-06-18 | Each selected row must display a distinct marker symbol to indicate its selected state. |
| R-06-19 | Each selected row must have a visually differentiated background that is clearly distinguishable from unselected rows but not so prominent as to obscure file information. |
| R-06-20 | When one or more files are selected, the application must display the total selection count in a prominent, always-visible location (e.g., the title area). |

## Acceptance criteria

### AC-06-01: Toggle a single file
- **Given** a file list with the cursor on an unselected file
- **When** the user presses the toggle key
- **Then** the file is added to the selection and its row displays the selection marker and differentiated background.

### AC-06-02: Untoggle a selected file
- **Given** a file list with the cursor on a selected file
- **When** the user presses the toggle key
- **Then** the file is removed from the selection and its row reverts to normal styling.

### AC-06-03: Range sweep down
- **Given** the cursor is on row 5 (unselected) in a list
- **When** the user initiates a range sweep downward three times
- **Then** rows 5, 6, 7, and 8 are all selected, and the cursor rests on row 8.

### AC-06-04: Range sweep direction reversal
- **Given** the user has swept downward, selecting rows 5–8
- **When** the user reverses and sweeps upward once
- **Then** row 8 is deselected (the reversal toggles both origin and landing), and the cursor moves to row 7.

### AC-06-05: Jump to group start
- **Given** the cursor is on the third file of a group
- **When** the user presses the group-start jump key
- **Then** the cursor moves to the first file of that group.

### AC-06-06: Jump to group start when already at start
- **Given** the cursor is on the first file of a group (not the first group overall)
- **When** the user presses the group-start jump key
- **Then** the cursor moves to the first file of the previous group.

### AC-06-07: Jump to next group
- **Given** the cursor is anywhere in a group that is not the last group
- **When** the user presses the next-group jump key
- **Then** the cursor moves to the first file of the next group.

### AC-06-08: Jump to next group at last group
- **Given** the cursor is in the last group of the list
- **When** the user presses the next-group jump key
- **Then** the cursor does not move (no-op).

### AC-06-09: Range select to group start (all-or-nothing)
- **Given** the cursor is in the middle of a group and none of the files between cursor and group start are selected
- **When** the user performs a modified group-start range selection
- **Then** all files from the cursor to the group start (inclusive) are selected.

### AC-06-10: Range select to group start when all already selected
- **Given** the cursor is in the middle of a group and every file from cursor to group start is already selected
- **When** the user performs a modified group-start range selection
- **Then** all files from the cursor to the group start are deselected.

### AC-06-11: Range select to group end with cursor overshoot
- **Given** the cursor is in the middle of a group
- **When** the user performs a modified group-end range selection
- **Then** all files from the cursor to the last file in the group are selected, and the cursor advances to the first file of the next group.

### AC-06-12: Select all
- **Given** a filtered result set with some files unselected
- **When** the user presses the select-all shortcut
- **Then** every file in the result set is selected and the selection count updates.

### AC-06-13: Deselect all via same shortcut
- **Given** every file in the result set is already selected
- **When** the user presses the select-all shortcut
- **Then** all files are deselected and the selection count is hidden.

### AC-06-14: Escape clears selection first
- **Given** the user has a non-empty selection, an open preview, and an active filter
- **When** the user presses the escape key once
- **Then** the selection is cleared, but the preview and filter remain active.

### AC-06-15: Cursor fallback when selection is empty
- **Given** no files are selected and the cursor is on a specific file
- **When** the user triggers a bulk operation (e.g., trash, tag)
- **Then** the operation targets only the file under the cursor.

### AC-06-16: Selection auto-clears after bulk operation
- **Given** three files are selected
- **When** the user executes a bulk operation that succeeds
- **Then** after the operation completes, the selection is empty and the selection count is no longer displayed.

### AC-06-17: Selection count in title
- **Given** 7 files are selected
- **When** the user looks at the title area
- **Then** the selection count "7" (or equivalent textual indicator) is prominently visible.

## Success metrics

- Range sweep selection of 100 consecutive files completes in under one second of key-repeat input.
- Users can select an entire group with at most two key presses (group-start jump + range select to group end, or vice-versa).
- Zero instances of stale selection state after a successful bulk operation.

## Constraints

- Group boundaries are determined solely by the application's filename convention; no user-configurable grouping is required for this feature.
- The selection model operates on the entire selection set. Files hidden by an active filter remain part of the selection and are affected by bulk actions.
- Selection size is unbounded, but bulk operations must perform safety pre-checks (e.g., batch limits) before execution.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-04  | Selection |
| PRD-00 | FR-4 (multi-selection fallback), FR-6 (escape hierarchy) |
