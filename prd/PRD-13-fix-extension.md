# PRD-13 — Fix File Extension

**Status**: `Draft`
**Derived from**: UC-10 (Fix file extension)

---

## 1. Purpose

Ensure file extensions accurately reflect the underlying file format by probing file signatures (magic bytes) and renaming files with incorrect extensions.

## 2. Requirements

### 2.1 Extension Validation

| ID | Requirement |
|----|-------------|
| R-13-01 | The system must provide a command to validate and fix file extensions for selected files. |
| R-13-02 | The system must read the header (magic bytes) of each targeted file to determine its actual format. |
| R-13-03 | The system must support detection for common image formats (e.g., JPEG, PNG, GIF, BMP, WebP). |
| R-13-04 | If the file's actual format cannot be confidently detected (e.g., raw formats, video files), the system must leave the file unchanged. |

### 2.2 Correction & Renaming

| ID | Requirement |
|----|-------------|
| R-13-05 | If the detected format's canonical extension does not match the file's current extension, the system must rename the file on disk to use the correct extension. |
| R-13-06 | Only the extension must be changed; the rest of the filename must remain completely unaltered. |
| R-13-07 | Files that already have the correct extension must be silently skipped. |
| R-13-08 | Upon completion, the system must report a summary of files fixed, files already correct, and any errors encountered. |

## 3. Acceptance Criteria

### AC-13-01: Correct mismatched extension
- **Given** a JPEG image incorrectly named `photo.png`
- **When** the fix extension command is executed on the file
- **Then** the file is renamed to `photo.jpg` on disk and the library is updated.

### AC-13-02: Ignore correct extensions
- **Given** a PNG image correctly named `graphic.png`
- **When** the fix extension command is executed
- **Then** the file is left unchanged and reported as already correct.

### AC-13-03: Ignore unknown formats
- **Given** a proprietary video file whose format the system does not probe
- **When** the fix extension command is executed
- **Then** the file is silently skipped and left unchanged.
