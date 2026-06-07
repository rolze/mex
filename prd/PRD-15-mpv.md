# PRD-15 — Video Player Integration

**Status**: `Draft`
**Derived from**: UC-13 (mpv integration)

---

## 1. Purpose

Allow the user to play and control video files via an external, remote-controlled video player without leaving the main terminal interface, ensuring a seamless, uninterrupted browsing experience.

## 2. Requirements

### 2.1 Video Playback

| ID | Requirement |
|----|-------------|
| R-15-01 | The system must allow the user to open a video file in the external video player using a dedicated keybinding. |
| R-15-02 | The system must validate the file extension before launch; non-video files must result in an error message. |
| R-15-03 | If the video player is not running, the system must spawn it automatically as a separate process and wait for it to be ready. |
| R-15-04 | The system must not suspend the main interface while the video player is running; the user must be able to continue browsing. |
| R-15-05 | The system must support establishing an Inter-Process Communication (IPC) socket with the video player to send commands and receive state updates. |

### 2.2 Remote Control

| ID | Requirement |
|----|-------------|
| R-15-06 | The system must provide keyboard shortcuts in the main interface to control the active video player (e.g., play/pause, next track, previous track). |
| R-15-07 | When the user triggers "next" or "previous", the system must find the next/previous video file in the currently filtered list and instruct the player to load it. |
| R-15-08 | If the user was paused when switching tracks, the new track must also load in a paused state. Otherwise, it must auto-play. |

### 2.3 Status Feedback

| ID | Requirement |
|----|-------------|
| R-15-09 | The system must continuously display the real-time state of the video player (e.g., stopped, playing filename, paused filename) in a dedicated status area. |
| R-15-10 | If the video player executable cannot be found or fails to launch, a clear error message must be displayed. |
| R-15-11 | The system must cleanly shut down its IPC connection when the application exits, leaving the video player running if the user was actively watching. |

## 3. Acceptance Criteria

### AC-15-01: Launch video player
- **Given** the cursor is on a valid video file and the player is not running
- **When** the user presses the play shortcut
- **Then** the video player is launched as a background process and the file begins playing.

### AC-15-02: Remote control from main interface
- **Given** a video is currently playing
- **When** the user presses the pause shortcut in the main interface
- **Then** the video pauses, and the status area updates to reflect the paused state.

### AC-15-03: Next track navigation
- **Given** a video is playing
- **When** the user presses the "next track" shortcut
- **Then** the system identifies the next video file in the list and instructs the player to switch to it instantly, bypassing non-video files.
