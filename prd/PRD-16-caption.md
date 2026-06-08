# PRD-16 — Caption Files

**Status**: `Draft`
**Derived from**: UC-14 (Caption)



## 1. Purpose

Enable the user to attach short, descriptive text to media files. Captions are embedded directly into the filename, adhering to a strict convention, ensuring portability and eliminating the need for sidecar files.

## 2. Requirements

### 2.1 Editing Captions

| ID | Requirement |
|----|-------------|
| R-16-01 | The system must provide a command to add or edit the caption of the currently focused file. |
| R-16-02 | Activating the caption command must open an inline editor that visually replaces the caption segment of the filename in the file list. |
| R-16-03 | If the file already has a caption, the editor must pre-load the existing text and place the cursor at the end, allowing the user to append characters without clearing it. |
| R-16-04 | The system must provide a quick way (e.g., a specific key) to instantly clear the entire caption within the editor. |

### 2.2 Input Validation & Transliteration

| ID | Requirement |
|----|-------------|
| R-16-05 | The system must automatically transliterate specific characters as the user types (e.g., spaces to hyphens, accented characters to ASCII equivalents like 'ä' to 'ae'). |
| R-16-06 | The system must reject unsupported characters (allowing only alphanumerics and hyphens) and display a warning. |
| R-16-07 | The system must enforce a maximum length for captions (e.g., 42 characters) and display a character counter during editing. |

### 2.3 Applying Captions

| ID | Requirement |
|----|-------------|
| R-16-08 | Upon confirmation, the system must safely extract the components of the original filename, inject the new caption, and rebuild the filename according to strict formatting rules. |
| R-16-09 | The system must rename the file on the filesystem and update its record in the database. |
| R-16-10 | If the newly generated filename conflicts with an existing file, the system must automatically append or increment a numerical counter to resolve the collision safely. |
| R-16-11 | If the user confirms an empty caption input, the system must remove the caption segment entirely from the filename. |
| R-16-12 | When a caption is added or removed, the file's existing 4-digit sequential counter must be preserved. The system must not attempt to condense or recalculate counters to fill gaps. |

## 3. Acceptance Criteria

### AC-16-01: Auto-transliteration during input
- **Given** the inline caption editor is active
- **When** the user types "Beach Photo ö"
- **Then** the input is instantly transformed to "Beach-Photo-oe".

### AC-16-02: Renaming with collision resolution
- **Given** two different images taken on the same day
- **When** the user applies the exact same caption to both images (sequentially)
- **Then** the system assigns the caption to the first file, and assigns the caption with an incremented counter to the second file, preventing an overwrite.

### AC-16-03: Removing a caption
- **Given** a file with an existing caption
- **When** the user opens the caption editor, clears the input, and confirms
- **Then** the caption segment is removed from the file's name on disk.


