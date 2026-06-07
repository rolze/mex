# PRD-02 · Browse & Navigate File List

| Meta | Data |
|------|------|
| **Status** | `Draft` |
| **Derived from** | UC-02 (Browse Media), UC-17 (Navigation Groups) |

---

## Problem

Users need to browse, navigate, and filter large media collections quickly. The file list is the primary surface of the application — it must render clearly, respond instantly to keyboard input, and give the user continuous orientation through title bar position counters, status messages, and visual cues for missing files. This PRD covers *only* the file list panel, its columns, cursor movement, text filtering, date-group jumping, and informational bars. Selection, tag/type filtering, file details, and external viewer launching are covered by separate PRDs.

## User stories

- As a user, I want to see my media files in a structured, colour-coded list with folder, filename, and tag columns, so that I can scan the collection at a glance.
- As a user, I want to navigate the list with keyboard controls (row, page, half-page, first/last, date-group jump), so that I can move through large collections without lag.
- As a user, I want to type a live text filter to narrow the list instantly, with wildcard support, so that I can locate files by partial name.
- As a user, I want the title bar to tell me my position in the list and how many files match the current filter, so that I always know where I am.
- As a user, I want files that are missing from disk to be clearly marked, so that I can spot integrity problems without opening each file.

---

## Requirements

### 2.1 File List Display

| ID | Requirement |
|----|-------------|
| R-02-01 | The application must display a file list with three fixed-width columns: folder (6 characters), filename (remaining width, front-truncated), and tags (30 characters, end-truncated). |
| R-02-02 | Files without tags must display a dash placeholder (`—`) in the tag column. |
| R-02-03 | Adjacent rows in the file list must use alternating background shading to improve scanability. |
| R-02-04 | Filename text must use semantic colour-coding to visually distinguish structural parts of the filename (e.g. dates, topics, subjects). |
| R-02-05 | The slug and caption portions of the filename must be highlighted distinctly from surrounding text. |
| R-02-06 | Files must be sorted by path, with collision-aware ordering: a base file must appear immediately before its collision siblings (e.g. `name.ext` before `name-2.ext`). |

### 2.2 Cursor Navigation

| ID | Requirement |
|----|-------------|
| R-02-07 | The user must be able to move the cursor one row at a time (up / down). |
| R-02-08 | The user must be able to move the cursor by a full visible page (page up / page down). The page size must equal the number of rows currently visible in the file list. |
| R-02-09 | The user must be able to move the cursor by half a visible page (half-page up / half-page down). |
| R-02-10 | The user must be able to jump the cursor to the first or last item in the list. |
| R-02-11 | The list must auto-scroll to keep the cursor row visible after every navigation action. |
| R-02-12 | The user must be able to jump to the next or previous date group boundary (the first file whose date-folder differs from the current file's date-folder). |

### 2.3 Text Filter

| ID | Requirement |
|----|-------------|
| R-02-13 | The user must explicitly activate filter mode via a dedicated key before text input is accepted, preventing accidental filtering during normal navigation. |
| R-02-14 | While filter mode is active, printable characters typed by the user must feed a live substring filter that updates the visible file list after every keystroke. |
| R-02-15 | The text filter must be case-insensitive. |
| R-02-16 | The text filter must support a wildcard character (`*`) that matches one or more arbitrary characters within the filename. |
| R-02-17 | During active filtering, the characters in the filename column that match the filter pattern must be highlighted. |
| R-02-18 | The filter input area must visually indicate when filter mode is active (e.g. highlighted border). |
| R-02-19 | Exiting filter mode must clear the filter text and restore the full, unfiltered list. |

### 2.4 Title Bar & Status Bar

| ID | Requirement |
|----|-------------|
| R-02-20 | The title bar must display the current cursor position and total item count in the format `pos / total`. |
| R-02-21 | When a text filter is active, the title bar must display `pos / filtered / total` to show both the filtered count and the full collection size. |
| R-02-22 | When one or more items are selected (see PRD-06), the title bar must additionally display the selection count, e.g. `(N selected)`. |
| R-02-23 | The status bar must display contextual feedback messages (e.g. confirmation of actions, hints for next steps). Status messages must be right-aligned and must not overwrite the filter input text. |

### 2.5 Missing File Indicators

| ID | Requirement |
|----|-------------|
| R-02-24 | Files whose corresponding physical file no longer exists on disk must be displayed with a distinct marker (e.g. `!` prefix) and a visually degraded style (e.g. muted or reddish tint). |
| R-02-25 | The missing-file status must be checked lazily — only when the user previews or interacts with the file — not eagerly for the entire list. |
| R-02-26 | Once a file is determined to be missing, that status must be persisted so that the indicator remains visible without re-checking on every display. |

### 2.6 Performance

| ID | Requirement |
|----|-------------|
| R-02-27 | Cursor navigation, scrolling, and text filtering must be perceptually instant (no visible lag) on lists containing up to 50,000 entries. |

---

## Acceptance Criteria

### AC-02-01: File list columns render correctly
- **Given** an indexed collection of media files
- **When** the application opens
- **Then** the file list displays three columns — folder (6 chars), filename (remaining width, front-truncated), and tags (30 chars, end-truncated) — with alternating row shading and semantic filename colour-coding.

### AC-02-02: Collision-aware sort order
- **Given** a collection containing `photo.jpg` and `photo-2.jpg`
- **When** the file list is sorted
- **Then** `photo.jpg` appears immediately before `photo-2.jpg`, regardless of byte-order sort.

### AC-02-03: Cursor movement by row, page, and half-page
- **Given** a file list with more entries than fit on one screen
- **When** the user navigates by single row, full page, or half page
- **Then** the cursor moves the expected distance and the list auto-scrolls to keep the cursor visible.

### AC-02-04: Jump to first and last
- **Given** a file list with the cursor somewhere in the middle
- **When** the user jumps to the first item
- **Then** the cursor moves to the first row; when the user jumps to the last item, the cursor moves to the last row.

### AC-02-05: Date group jump
- **Given** a file list with files spanning multiple date folders
- **When** the user triggers "next date group"
- **Then** the cursor jumps to the first file in the next date-folder group; "previous date group" jumps to the first file of the previous group.

### AC-02-06: Live text filter with highlighting
- **Given** the user activates filter mode and types a substring
- **When** the filter text changes
- **Then** the file list instantly updates to show only matching files, and the matching characters in each filename are highlighted.

### AC-02-07: Wildcard filter
- **Given** an active filter containing `*`
- **When** the user types e.g. `cat*dog`
- **Then** only files whose names contain "cat" followed (at any distance) by "dog" are shown, with the matched literal segments highlighted.

### AC-02-08: Filter mode visual cue
- **Given** filter mode is active
- **When** the user looks at the filter input area
- **Then** the border or background of the filter area is visually distinct from its inactive state.

### AC-02-09: Title bar position counter
- **Given** a list of 500 files with no filter active and the cursor on row 42
- **When** the title bar is rendered
- **Then** it reads `42 / 500`.

### AC-02-10: Title bar with active filter
- **Given** a filter that narrows 500 files to 37 matches, cursor on row 5
- **When** the title bar is rendered
- **Then** it reads `5 / 37 / 500`.

### AC-02-11: Missing file indicator
- **Given** a file in the list whose physical file has been deleted from disk
- **When** the user previews that file (triggering a lazy check)
- **Then** the file's row gains a distinct missing-file marker and degraded styling, and this status persists on subsequent views without re-checking.

### AC-02-12: Performance on large lists
- **Given** a list of 50,000 entries
- **When** the user navigates or filters
- **Then** all interactions remain perceptually instant with no visible lag.

---

## Constraints

- Filename truncation must preserve the tail of the filename (extension and trailing characters), removing characters from the front.
- Status messages must not overwrite the active filter text in the input area.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-02 | Browse Media |
| UC-17 | Navigation Groups / Semantic Zoom (date-group jump; zoom covered in PRD-04) |

## Out of Scope

The following concerns are deliberately excluded from this PRD and covered elsewhere:

| Topic | PRD |
|-------|-----|
| Semantic zoom (Left/Right collapse/expand) | PRD-04 |
| File details panel | PRD-05 |
| File selection & multi-select | PRD-06 |
| Tag and type filtering (`#`, `@`) | PRD-07 |
| External viewer launching | PRD-08 |
