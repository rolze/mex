# PRD-14 — Trash & Delete

**Status**: `Draft`
**Derived from**: UC-11 (Trash and delete)



## 1. Purpose

Provide a safe, two-step deletion process to remove unwanted media. Files are first "soft-deleted" (trashed) where they can be restored, and later permanently deleted from disk in guarded batches.

## 2. Requirements

### 2.1 Trashing Files (Soft Delete)

| ID | Requirement |
|----|-------------|
| R-14-01 | The system must allow the user to mark a file as trashed using a dedicated keybinding. |
| R-14-02 | Trashing a file must operate strictly on the cursor item, deliberately ignoring any active multi-selection to protect the user from accidental bulk trashing. |
| R-14-03 | Trashing must be a logical operation (updating status) without moving or deleting the physical file on disk. |
| R-14-04 | The system must automatically assign a dedicated system tag to trashed files to allow easy filtering. |
| R-14-05 | Trashed files must remain visible in the file list but must be styled distinctly (e.g., dimmed, with an icon) and cannot be selected for bulk operations. |

### 2.2 Restoring Files (Keep)

| ID | Requirement |
|----|-------------|
| R-14-06 | The system must allow the user to restore a trashed file back to normal status using a dedicated keybinding. |
| R-14-07 | Restoring a file must remove the dedicated system tag and return its visual styling to normal. |

### 2.3 Permanent Deletion (Empty Trash)

| ID | Requirement |
|----|-------------|
| R-14-08 | The system must provide a command to permanently delete trashed files. |
| R-14-09 | Before permanent deletion, the system must list the files to be destroyed and require explicit user confirmation within the Multipurpose Context-Aware Panel. Full-screen overlays must not be used. |
| R-14-10 | Permanent deletion must process files in guarded batches (e.g., maximum 100 files per invocation) to prevent accidental mass data loss. |
| R-14-11 | Upon confirmation, files must be permanently deleted from the filesystem. |
| R-14-12 | The system must retain a permanent record of the deleted file's content hash to prevent the file from being re-imported in the future (deduplication guard). |

### 2.4 Status Indicators

| ID | Requirement |
|----|-------------|
| R-14-13 | If the trash contains files, the system must display a non-obtrusive indicator in the interface showing the total number of trashed items. |

## 3. Acceptance Criteria

### AC-14-01: Soft-delete operation
- **Given** a normal file in the library
- **When** the user invokes the trash action
- **Then** the file is visually marked as trashed, receives the system trash tag, but remains on disk.

### AC-14-02: Restore operation
- **Given** a previously trashed file
- **When** the user invokes the restore action
- **Then** the file loses its trashed status and system tag, returning to a normal interactive state.

### AC-14-03: Empty trash confirmation
- **Given** multiple trashed files
- **When** the user invokes the empty trash command
- **Then** a warning screen lists the files and requires a yes/no confirmation before any files are deleted.

### AC-14-04: Deduplication guard retention
- **Given** a file that has been permanently deleted via the empty trash command
- **When** the user attempts to re-import the exact same file
- **Then** the import process recognizes it as a duplicate (via retained hash) and skips importing it.


