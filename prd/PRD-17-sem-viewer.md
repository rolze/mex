# PRD-17 — Image Viewer

**Status**: `Draft`
**Derived from**: UC-15 (sem Image Viewer)

---

## 1. Purpose

Provide a fast, integrated external image viewer specifically designed to handle high-resolution single images or display a grid of thumbnails for bulk-selected files, seamlessly launched from the main terminal interface.

## 2. Requirements

### 2.1 Single Image View

| ID | Requirement |
|----|-------------|
| R-17-01 | The system must launch the external image viewer as a separate, detached process when the user attempts to view a single image or an empty selection. |
| R-17-02 | The viewer must display the image scaled to fit the window, alongside its filename and associated tags. |
| R-17-03 | The main terminal interface must remain fully interactive while the image viewer is running. |

### 2.2 Grid View (Multiple Images)

| ID | Requirement |
|----|-------------|
| R-17-04 | If the user selects two or more images and triggers the view command, the system must generate a temporary manifest file containing the paths and tags of the selected images. |
| R-17-05 | The system must launch the viewer in grid mode, passing the manifest file as an argument. |
| R-17-06 | In grid mode, the viewer must display a scrollable grid of thumbnails for the provided images. |
| R-17-07 | The viewer must utilize a dedicated cache directory to store and retrieve generated thumbnails to ensure fast loading times. |
| R-17-08 | Clicking a thumbnail in grid mode must transition the viewer to the full-size single-image view for that specific file. |
| R-17-09 | The user must be able to return to the grid view from the single-image view. |

### 2.3 Error Handling

| ID | Requirement |
|----|-------------|
| R-17-10 | If the view command is triggered on a non-image file (and not a supported video file), the system must display an error message and not launch the viewer. |

## 3. Acceptance Criteria

### AC-17-01: Single image launch
- **Given** the cursor is on an image file with no active multi-selection
- **When** the user presses the view command
- **Then** the external image viewer opens displaying the full-size image, and the terminal remains interactive.

### AC-17-02: Grid view generation
- **Given** the user has selected 5 image files
- **When** the user presses the view command
- **Then** the system creates a manifest of those 5 files and opens the viewer displaying a grid of 5 thumbnails.

### AC-17-03: Grid to single transition
- **Given** the viewer is open in grid mode
- **When** the user clicks a thumbnail
- **Then** the viewer transitions to show the full-size version of that specific image.
