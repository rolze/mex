# PRD-18 — Filename Slugification

**Status**: `Draft`
**Derived from**: UC-16 (Slugify & Deslugify)

---

## 1. Purpose

Provide tools to automatically format, group, and normalize filenames using "slugs" (concise, kebab-case contextual words) to ensure consistency and readability across the library, and to repair incorrectly grouped files.

## 2. Requirements

### 2.1 Deslugify (Repair)

| ID | Requirement |
|----|-------------|
| R-18-01 | The system must provide a command to remove existing slugs from selected files and reset them to a standard date-based format. |
| R-18-02 | The system must pre-calculate and assign chronological counters for the entire batch before executing any renames, preventing counter collisions. |
| R-18-03 | The system must extract the existing slug from the filename before removal and save it as a distinct "slug" tag in the database to prevent data loss. |
| R-18-04 | The system must perform the operation idempotently; files that already lack a slug must be silently skipped. |
| R-18-05 | The operation must run in a background thread, displaying a full-screen progress overlay that blocks other input until completion. |

### 2.2 Slugify (Assign/Group)

| ID | Requirement |
|----|-------------|
| R-18-06 | The system must provide a command to group selected files under a new or existing slug. |
| R-18-07 | The input for the slug must be normalized in real-time as the user types (e.g., spaces to hyphens, special characters transliterated to ASCII, invalid characters rejected). |
| R-18-08 | The system must enforce a minimum length (e.g., 3 characters) and maximum length for the slug. |
| R-18-09 | The system must validate that all selected files share the same year-month prefix before allowing them to be grouped under a single slug. |
| R-18-10 | The system must validate that the selection does not contain files that already belong to two or more different slugs. |

### 2.3 Slugify Execution Modes

| ID | Requirement |
|----|-------------|
| R-18-11 | **Rename Mode:** If all selected files already share the exact same slug, the system must simply swap the old slug for the new slug in the filename without changing existing numerical counters. |
| R-18-12 | **Assign Mode:** For all other valid selections, the system must calculate sequential counters for the target group, ensuring chronological order and avoiding collisions with existing files. |
| R-18-13 | The operation must run in a background thread with a progress overlay, blocking input until complete. |

## 3. Acceptance Criteria

### AC-18-01: Deslugify preserves context
- **Given** a file named `2023-05-party-0001.jpg`
- **When** the deslugify command is executed
- **Then** the file is renamed to a date-only format (e.g., `2023-05-15-0001.jpg`) and the tag "party" is added to the file's metadata.

### AC-18-02: Slug input normalization
- **Given** the user invokes the slugify command
- **When** the user types "My Event!"
- **Then** the input is instantly normalized to "my-event" and the exclamation mark is rejected.

### AC-18-03: Multi-month guardrail
- **Given** the user selects one file from May and one file from June
- **When** the user attempts to apply a slug to both
- **Then** the command is rejected with an error stating files must share the same month.

### AC-18-04: Rename mode preserves counters
- **Given** files `2023-05-trip-0001.jpg` and `2023-05-trip-0002.jpg` are selected
- **When** the user applies the new slug "vacation"
- **Then** the files are renamed to `2023-05-vacation-0001.jpg` and `2023-05-vacation-0002.jpg` exactly, preserving the 0001/0002 counters.
