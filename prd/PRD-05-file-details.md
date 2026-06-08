# PRD-05 · File Details Panel

| Meta | Data |
|------|------|
| **Status** | `Draft` |
| **Derived from** | UC-03 (File Details) |

## Problem

When browsing a large media collection, the file list alone shows only the filename and tags. Users need a way to inspect the full metadata of the selected file — path, original filename, date, slug, caption, and tags — at a glance, alongside an inline image preview, without leaving the list view.

## User stories

- As a user, I want to toggle a details panel next to the file list, so that I can see the full metadata and a visual preview of the selected file without opening an external viewer.
- As a user, I want the metadata header to present key fields in a compact, structured layout, so that I can scan file context quickly.
- As a user, I want long paths and filenames to be intelligently truncated so that they always fit within the available space, even after terminal resize.
- As a user, I want absent metadata fields to show a clear placeholder rather than blank space, so that I can distinguish "no value" from "not loaded."

## Requirements

### Functional requirements — Panel toggling

| ID | Requirement |
|----|-------------|
| R-05-01 | The user must be able to toggle the details panel open and closed using the `Enter` key. (`Space` is reserved for selection). |
| R-05-02 | When open, the details panel must appear as a right-side pane alongside the file list. The panel must always start closed when the application launches. |
| R-05-02b | If a background task is actively running, pressing `Enter` must be a no-op, as the right pane is occupied by the task. |
| R-05-02c | The application must refuse to open the details panel if the total terminal width is less than 100 columns, to avoid an unusably narrow file list. |
| R-05-03 | When closed, the full width must be returned to the file list. |

### Functional requirements — Metadata header

| ID | Requirement |
|----|-------------|
| R-05-04 | The metadata header must use a two-column layout occupying exactly three lines. |
| R-05-05 | The left column must display, top to bottom: the full relative file path (labelled "File"), the file's date and time from the data store (labelled with the date/time), and the original filename (labelled "Orig"). |
| R-05-06 | The right column must display, top to bottom: the file's tags (visually separated by dividers), the slug, and the caption. |
| R-05-07 | Tags must be displayed in a visually distinct style. Each tag must be separated from the next by a divider character. |
| R-05-08 | The slug must be displayed in a visually distinct style that differentiates it from other metadata fields. |
| R-05-09 | The caption must be displayed in a visually distinct style that differentiates it from both tags and slug. |
| R-05-10 | Every metadata field must display a dash placeholder (`—`) when the value is absent. Fields must never appear blank. |

### Functional requirements — Filename highlighting

| ID | Requirement |
|----|-------------|
| R-05-11 | In the file list's filename column, the slug portion of the filename must be visually highlighted in a distinct style. |
| R-05-12 | In the file list's filename column, the caption portion of the filename must be visually highlighted in a distinct style that is different from the slug highlight. |
| R-05-13 | Slug and caption highlights must be applied simultaneously when both are present in the filename. |
| R-05-14 | When neither slug nor caption is present in the filename, no semantic highlighting must be applied. |

### Functional requirements — Truncation

| ID | Requirement |
|----|-------------|
| R-05-15 | When the File or Orig value exceeds the available column width, the value must be front-truncated with a leading ellipsis character (`…`) so that the tail (most distinctive part) remains visible. |
| R-05-16 | Truncation must be recalculated whenever the terminal is resized, so that values always fit within the current column width. |

### Functional requirements — Inline image preview

| ID | Requirement |
|----|-------------|
| R-05-17 | Below the metadata header, the remaining height of the details pane must be used to display an inline image preview of the selected file. If the terminal does not support inline graphics, the preview must degrade gracefully and show a placeholder message. |
| R-05-18 | The image preview must scale to fit the available pane area without cropping. |

### Functional requirements — List title

| ID | Requirement |
|----|-------------|
| R-05-19 | The list title must always display the current cursor position and total item count. |
| R-05-20 | When a filter is active, the list title must additionally display the filtered item count between the position and total. |

### Non-functional requirements

| ID | Requirement |
|----|-------------|
| R-05-21 | Opening or closing the details panel must be perceptually instant (no visible lag). |
| R-05-22 | Truncation recalculation on resize must be perceptually instant. |

## Acceptance criteria

### AC-05-01: Toggle details panel open
- **Given** the details panel is closed and no background task is running
- **When** the user presses `Enter`
- **Then** a right-side details pane opens showing the metadata header and image preview for the selected file

### AC-05-02: Toggle details panel closed
- **Given** the details panel is open
- **When** the user presses `Enter` again
- **Then** the details pane closes and the file list reclaims the full width

### AC-05-03: Metadata header layout
- **Given** the details panel is open
- **When** the user views the metadata header
- **Then** it displays exactly three lines in two columns: left column shows File, Date+Time, and Orig; right column shows Tags, Slug, and Caption

### AC-05-04: Absent fields show placeholder
- **Given** a file with no tags, no slug, and no caption
- **When** the details panel is open for that file
- **Then** the Tags, Slug, and Caption fields each display `—`

### AC-05-05: Tag display with dividers
- **Given** a file with multiple tags
- **When** the details panel is open
- **Then** each tag is shown in a visually distinct style, separated from adjacent tags by a divider character

### AC-05-06: Slug and caption highlighting in filename column
- **Given** a file whose filename contains both a slug and a caption
- **When** the file is displayed in the list
- **Then** the slug portion is highlighted in one distinct style and the caption portion is highlighted in a different distinct style, both applied simultaneously

### AC-05-07: No highlighting when slug and caption absent
- **Given** a file whose filename contains neither a slug nor a caption
- **When** the file is displayed in the list
- **Then** no semantic highlighting is applied to the filename

### AC-05-08: Front-truncation of long paths
- **Given** the File value exceeds the available column width
- **When** the metadata header is rendered
- **Then** the value is front-truncated with a leading `…`, preserving the tail of the path

### AC-05-09: Truncation updates on resize
- **Given** the details panel is open and displaying a truncated File value
- **When** the terminal is resized to a wider width
- **Then** the truncation is recalculated and more of the path becomes visible

### AC-05-10: Image preview fills remaining space
- **Given** the details panel is open and the terminal supports inline graphics
- **When** the metadata header occupies its three lines
- **Then** the remaining pane height displays an inline image preview scaled to fit

### AC-05-11: List title without filter
- **Given** no filter is active
- **When** the user views the list title
- **Then** it displays the current cursor position and total item count

### AC-05-12: List title with active filter
- **Given** a filter is active reducing the visible list
- **When** the user views the list title
- **Then** it displays the cursor position, the filtered count, and the total count

## Success metrics

- Users can assess all key metadata (path, date, original name, tags, slug, caption) within 2 seconds of opening the details panel.
- Truncated values always fit within their column — no layout overflow or line wrapping under any terminal width ≥ 80 columns.

## Constraints

- The metadata header is fixed at three lines; it must not grow or scroll.
- Front-truncation (preserving the tail) is required for File and Orig; end-truncation is not acceptable for these fields.
- The file extension is intentionally omitted from the metadata header, as it is already visible in the filename column.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-03  | mex/spec/UC-03-file-details.md |
