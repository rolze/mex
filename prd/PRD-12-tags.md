# PRD-12 — Assign & Remove Tags

**Status**: `Draft`
**Derived from**: UC-09 (Assign / remove tags)

---

## 1. Purpose

Allow the user to apply or remove contextual tags (and tag types) on single files or bulk selections, enabling rich organization and filtering capabilities.

## 2. Requirements

### 2.1 Assigning Tags

| ID | Requirement |
|----|-------------|
| R-12-01 | The system must provide a command to assign a tag to the currently selected files (or cursor file if no selection). |
| R-12-02 | The assign command must support auto-completion of existing tag names. |
| R-12-03 | The user must be able to specify an optional tag type when assigning a tag. |
| R-12-04 | If a tag type is specified, the system must provide auto-completion of existing tag types. |
| R-12-05 | If a tag already exists and no type is specified, the system must reuse its existing type. |
| R-12-06 | If a new tag is created and no type is specified, the system must apply a default type. |
| R-12-07 | If the user attempts to assign an existing tag with a type that conflicts with its established type, the system must reject the operation with an error. |
| R-12-08 | The system must silently skip files that already have the specified tag. |

### 2.2 Removing Tags

| ID | Requirement |
|----|-------------|
| R-12-09 | The system must provide a command to remove tags from the currently selected files. |
| R-12-10 | If specific tag names are provided, only those tags must be removed. |
| R-12-11 | If no tag names are provided, the system must remove all tags from the targeted files. |
| R-12-12 | The remove command must support auto-completion of existing tag names, allowing multiple tags to be chained sequentially. |

### 2.3 Performance & Feedback

| ID | Requirement |
|----|-------------|
| R-12-13 | Bulk tag assignment and removal must be performed atomically to ensure consistency. |
| R-12-14 | Following a successful operation, the file list must refresh and a status message must summarize the number of files affected. |

## 3. Acceptance Criteria

### AC-12-01: Tag assignment auto-completion
- **Given** the tag "vacation" exists in the library
- **When** the user types the assign command and begins typing "vac"
- **Then** "vacation" is offered as an auto-completion suggestion.

### AC-12-02: Bulk tag assignment
- **Given** 5 files are selected, 2 of which already have the tag "holiday"
- **When** the user assigns the tag "holiday"
- **Then** the tag is added to the 3 untagged files, the other 2 are skipped, and the status reports 5 files tagged.

### AC-12-03: Tag type conflict prevention
- **Given** the tag "alice" exists with the type "person"
- **When** the user attempts to assign "alice" with the type "location"
- **Then** the operation is rejected with an error and no files are modified.

### AC-12-04: Clear all tags
- **Given** a file with 3 different tags
- **When** the user executes the remove command with no arguments
- **Then** all 3 tags are removed from the file.
