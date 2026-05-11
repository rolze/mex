# mex — Media Explorer: Use Cases (DRAFT)

> Terminal-based media browser written in Rust + Ratatui.
> LazyVim-style keyboard navigation, SQLite-backed index, EXIF/audio/video metadata, fuzzy filtering, tagging, and album views.

---

## UC-01 · Scan & Index a Directory

**Actor:** User  
**Goal:** Recursively scan a directory and build a local SQLite index of all media files with extracted metadata.

**Preconditions:** User provides a path (or the current working directory is used as default).

**Main Flow:**
1. User runs `mex` (or `mex /path/to/media`).
2. mex checks whether an existing index exists for that path.
3. mex recursively walks the directory, identifying files by extension (images, audio, video).
4. For each discovered file mex extracts:
   - **Images:** EXIF data (date taken, GPS, camera model, dimensions).
   - **Audio:** ID3/Vorbis tags (title, artist, album, year, duration, bitrate).
   - **Video:** Container metadata (duration, resolution, codec, frame rate).
5. Extracted metadata and file paths are upserted into the SQLite index (`~/.local/share/mex/index.db` or `.mex.db` in the scanned directory — configurable).
6. A progress indicator is shown during the scan.
7. After completion, the main browser view opens automatically.

**Alternate Flows:**
- **A1 – Re-scan:** User triggers a re-scan (`R`); only changed/new files are re-processed (mtime-based diffing).
- **A2 – Missing file:** If an indexed file is no longer present, it is marked as `missing` in the DB and visually distinguished in the UI.

**Acceptance Criteria:**
- `mex` opens without error on a directory containing mixed media.
- All JPEG/PNG/GIF/WEBP images, MP3/FLAC/OGG/AAC audio files, and MP4/MKV/AVI/MOV video files are discovered and indexed.
- Metadata extraction is non-blocking (runs in background threads); partial results are visible immediately.
- Re-running `mex` on a previously scanned directory is significantly faster than the initial scan.

---

## UC-02 · Browse Media Files

**Actor:** User  
**Goal:** Navigate the indexed media collection in the TUI with keyboard controls.

**Preconditions:** At least one directory has been scanned (UC-01).

**Main Flow:**
1. The main view displays a scrollable, sorted list of media files.
2. User navigates with:
   - `j` / `↓` — move selection down.
   - `k` / `↑` — move selection up.
   - `g` / `G` — jump to top / bottom.
   - `Ctrl-d` / `Ctrl-u` — half-page down / up.
   - `Enter` — open preview panel for selected file.
   - `q` / `Esc` — quit or close panel.
3. The list shows: filename, media type icon, duration/dimensions, date, and tag count.
4. Columns are sortable (`s` to cycle sort key: name, date, size, duration).

**Acceptance Criteria:**
- Navigation feels instant (no visible lag on lists up to 50 000 entries).
- Selected item is always visible (auto-scroll).
- Sort state is preserved across sessions (stored in the DB or config).

---

## UC-03 · Preview Metadata & Content

**Actor:** User  
**Goal:** Inspect the metadata of a selected file in a side/bottom panel.

**Preconditions:** A file is selected in the browser.

**Main Flow:**
1. User presses `Enter` or `p` to open the preview panel.
2. The panel renders:
   - **Images:** ASCII/Unicode art thumbnail (via `chafa` if available, else pixel-block fallback) + EXIF table.
   - **Audio:** Waveform hint (block characters) or album-art ASCII if embedded, + tag table.
   - **Video:** First-frame thumbnail attempt + metadata table.
3. The panel is closable and resizable (`|` to toggle split direction: vertical/horizontal).
4. Metadata fields are shown as a two-column table (key / value).

**Acceptance Criteria:**
- Panel opens within 200 ms for pre-indexed files (reads from DB, not disk).
- Graceful fallback when thumbnail tools are not available (shows metadata-only).
- Panel does not block navigation in the file list.

---

## UC-04 · Fuzzy Filter

**Actor:** User  
**Goal:** Quickly narrow down the file list by typing a fuzzy search query.

**Preconditions:** The browser is open with a populated list.

**Main Flow:**
1. User presses `/` to activate the filter bar at the bottom.
2. As the user types, the list is filtered in real time using fuzzy matching against:
   - File name
   - Directory path
   - Metadata fields (artist, title, album, camera model, etc.)
   - Tags
3. Matched characters in each result are highlighted.
4. User presses `Esc` to clear the filter and restore the full list, or `Enter` to confirm and keep the filtered view.
5. Filter query is shown in the status bar while active.

**Acceptance Criteria:**
- Filtering 50 000 entries feels responsive (< 50 ms per keystroke).
- Fuzzy algorithm ranks closer matches higher (filename prefix > substring > scattered).
- Clearing the filter restores scroll position to the previously selected item.

---

## UC-05 · Tag Files

**Actor:** User  
**Goal:** Attach free-form text tags to one or more files for later grouping.

**Preconditions:** Browser is open.

**Main Flow:**
1. User selects a file and presses `t` to open the tag editor popup.
2. Existing tags are shown as removable chips.
3. User types a new tag name; autocomplete suggests existing tags from the DB.
4. `Enter` adds the tag; `Backspace` removes the last chip; `Esc` cancels.
5. User can visually select multiple files (`Space` to toggle selection, `v` for range) and tag them all at once.
6. Tags are persisted to the SQLite index immediately.

**Alternate Flows:**
- **A1 – Remove tag:** User presses `T` (shift) to open a tag-removal popup with checkboxes.
- **A2 – Rename tag globally:** Command palette (`:`), `rename-tag <old> <new>` renames the tag across all files.

**Acceptance Criteria:**
- Tags appear in the file list view as coloured labels immediately after saving.
- Tag operations are atomic (no partial writes on crash).
- Autocomplete surfaces only tags already present in the DB.

---

## UC-06 · Album View (Tag-Based)

**Actor:** User  
**Goal:** View all files sharing a tag as a named "album" / collection.

**Preconditions:** At least one tag exists in the DB.

**Main Flow:**
1. User presses `A` to switch to Album View.
2. Left panel lists all tags (sorted by file count, then alphabetically).
3. Selecting a tag in the left panel populates the right panel with all files carrying that tag.
4. Navigation and preview work identically to the main browser (UC-02, UC-03).
5. User can press `A` again or `Esc` to return to the flat file browser.

**Acceptance Criteria:**
- Album View opens with no additional disk I/O (reads from the SQLite index).
- File counts per tag are accurate and update immediately after tagging.
- The same file can appear under multiple albums (many-to-many tags).

---

## UC-07 · File Actions

**Actor:** User  
**Goal:** Perform common file system operations from within the browser.

**Preconditions:** One or more files are selected.

**Main Flow:**
1. User presses `x` to open the action menu (or uses keybindings directly):
   - `o` — open with system default application (`xdg-open` / `open`).
   - `y` — yank (copy) path to clipboard.
   - `d` — delete (with confirmation prompt).
   - `m` — move to a directory (file-path input popup).
   - `c` — copy to a directory (file-path input popup).
   - `r` — rename (inline edit of the filename).
2. Destructive actions require a `y/n` confirmation.
3. After an action, the list refreshes and the index is updated accordingly.

**Acceptance Criteria:**
- `o` launches the correct application and returns focus to mex.
- `d` removes the file from disk **and** from the SQLite index.
- Move/copy operations update index paths correctly.
- All actions are undoable via a basic undo stack (`u` to undo last action) within the session.

---

## Non-Goals (v1)

- Network/cloud storage browsing.
- In-app media playback.
- Full image editing.
- Plugin system.
