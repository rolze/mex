# PRD-11 — Smart Import

**Status**: `Draft`
**Derived from**: UC-08 (Smart import)



## 1. Purpose

Provide a robust, idempotent mechanism to import new media files from external sources into the canonical target tree. The import process must deduplicate files, normalize filenames (date and slug), preserve original modification timestamps, and automatically tag imported files by session, all without blocking the main user interface.

## 2. Requirements

### 2.1 Command & Input

| ID | Requirement |
|----|-------------|
| R-11-01 | The system must provide a command to initiate import from a specified source directory path. |
| R-11-02 | The command input must support auto-suggestion of previously used source directories, ordered by recency. |
| R-11-03 | The system must validate the existence of the typed path in real-time and provide visual feedback (e.g., valid directory, not found, not a directory). |

### 2.2 Scan Phase

| ID | Requirement |
|----|-------------|
| R-11-04 | The system must recursively scan the source directory in the background. |
| R-11-05 | The scan must explicitly skip hidden files, metadata files, and known junk/cache directories. |
| R-11-06 | The scan must extract necessary metadata (size, modification time) without opening the file contents. |
| R-11-07 | The system must derive a canonical date for each file, prioritizing filename patterns, then folder structure, then filesystem modification time. |
| R-11-08 | The system must derive a normalized slug from the file's ancestor folder path, stripping out junk words and unsupported characters. |
| R-11-09 | The user must be able to safely abort the scan phase, instantly returning the system to its prior state. |

### 2.3 Preview Phase

| ID | Requirement |
|----|-------------|
| R-11-10 | Following the scan, the system must display a full-screen preview (dry run) of the proposed import. |
| R-11-11 | The preview must summarize total files found, pending imports, duplicates, skipped items, and items with unknown dates. |
| R-11-12 | The preview must list each file with its source path, proposed target path, date source, slug source, and status. |
| R-11-13 | The user must be able to scroll through the preview list and explicitly confirm or cancel the import execution. |

### 2.4 Execute Phase & Deduplication

| ID | Requirement |
|----|-------------|
| R-11-14 | Upon confirmation, the system must execute the import in the background without blocking the user interface. |
| R-11-15 | The system must assign collision-free filenames by querying existing files and assigning sequential counters for duplicates of the same date/slug. |
| R-11-16 | The system must perform deduplication by computing a partial content hash and comparing it against all known files (including previously deleted/trashed files). |
| R-11-17 | If a file is not a duplicate, the system must copy it to the target directory while simultaneously computing its full content hash. |
| R-11-18 | A full-screen progress overlay must be displayed, showing overall progress, current file, and providing an option to abort. |
| R-11-19 | If the user aborts during execution, the current file must finish copying, but no further files are processed. Completed imports must remain valid. |

### 2.5 Metadata Preservation & Tagging

| ID | Requirement |
|----|-------------|
| R-11-20 | The system must attempt to preserve the original modification time on the newly copied file, falling back gracefully if the filesystem rejects it. |
| R-11-21 | The system must automatically assign a unique session tag (e.g., `import-YY-MM-DD`) to all files successfully imported in that batch. |

### 2.6 Repair Command

| ID | Requirement |
|----|-------------|
| R-11-22 | The system must provide a separate repair command to re-apply the modification time logic to already imported files. |

## 3. Acceptance Criteria

### AC-11-01: Auto-suggest source path
- **Given** the user has previously imported from `/media/usb`
- **When** the user types the import command
- **Then** `/media/usb` is offered as an auto-completion suggestion.

### AC-11-02: Scan skips junk
- **Given** a source directory containing a `.thumbnails` folder and a `.txt` file
- **When** the scan phase runs
- **Then** those files and folders are ignored and do not appear in the preview.

### AC-11-03: Deduplication prevents redundant copies
- **Given** a file in the source directory that is an exact byte match for a file already in the library (even if named differently)
- **When** the import executes
- **Then** the file is marked as a duplicate and is not copied.

### AC-11-04: Collision-free filenames
- **Given** two different images taken on the same day with the same derived slug
- **When** the import executes
- **Then** the second file is assigned a sequential counter (e.g., `-0002`) to prevent overwriting the first.

### AC-11-05: Safe abort during execution
- **Given** an import of 100 files is executing in the background
- **When** the user aborts after 10 files
- **Then** the current file completes, files 1-10 are fully registered and tagged, and files 11-100 are untouched.

## Open questions

- Import deduplication skips files that exist in the trash. Should the system notify the user that skipped duplicates were found in the trash so they know they can restore them?
