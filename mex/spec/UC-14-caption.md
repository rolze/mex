## UC-14 · Add / Edit / Remove caption in filename

**Actor:** User  
**Goal:** Maintain a short and descriptive caption for file on cursor.

### Important reminder

According to the strict filename convention defined in `../doc/REGEXP.md`, each file can have an optional descriptive caption embedded in the filename before the extension. The inline editor manipulates this specific component.

### Flow

1. User positions the cursor on a file.
2. User presses `<F2>`.
   * The file's `target_path` is strictly parsed using the battle-tested regex from `REGEXP.md` to accurately detect and isolate the existing caption part.
   * This detected caption is loaded into an **inline editor** that visually replaces the caption segment of the filename in the file list.
   * **Pre-selection**: If an existing caption is present, it is initially highlighted. Unlike standard editors, typing **does not** clear the selection; instead, characters are appended to the existing text.
   * **Delete**: Pressing `<DEL>` clears the entire caption immediately.
   * **Backspace**: Removes the last character of the caption.
   * **On-the-fly Transliteration**: As the user types, special characters are converted immediately:
     * `Space` → `-`
     * `ä/ö/ü` → `ae/oe/ue`
     * `ß` → `ss`
   * **Validation**: Only `a-z`, `0-9`, and `-` are allowed. Unsupported characters are rejected with an "Invalid character for filename" warning in the status box.
   * **Visual Feedback**:
     * A counter `[current/42]` is displayed in the same row, immediately after the caption input.
     * A `_` cursor indicates the current typing position.
     * The filter bar shows: `F2: editing caption  —  ESC cancel  ·  ENTER confirm`.
   * User cancels with `<ESC>` or confirms with `<ENTER>`.
3. **Execution**: mex renames the physical file and updates `target_path`, `caption_slug`, and `counter` in the DB.
   * **Regex-based surgery**: The existing `target_path` is parsed using the regex from `REGEXP.md` to safely extract all structural components (year, month, day/slug, counter, existing caption, and extension).
   * The new caption is substituted into the parsed structure, and the filename is rebuilt cleanly according to its matched pattern format.
   * If the input was cleared, the caption segment and its preceding hyphen are omitted from the rebuilt filename.
   * `counter` is set to match the new path format: `NULL` for a plain day-caption file (`yyyy-mm-dd-caption.ext`), the collision N for a `caption-N` file, or the numeric counter for a day-counter or slug-counter file.
   * If the calculated filename already exists on disk or in the DB (by another live file), the maximum occupied counter is pre-computed (one `MAX(counter)` DB query + one filesystem scan), and the path is generated directly at `max + 1`, adhering strictly to the formats defined in `REGEXP.md`.

### Notes

- **Format**: Produces lowercase `kebab-case` captions (max 42 characters).
- **Persistence**: The cursor position is preserved after the rename.
- **Scope**: F2 is available in normal mode only (not while filter or command is active).