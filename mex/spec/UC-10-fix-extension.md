## UC-10 · Fix file extension

**Actor:** User  
**Goal:** Correct the file extension of one or more files whose extension does not match the actual format detected from magic bytes.

---

### Command

```
:fix-ext
```

No argument.  Operates on the explicitly selected files, or the cursor file if nothing is selected.

---

### Flow

1. User positions cursor or selects files with wrong extensions.
2. User types `:fix-ext` and presses Enter.
3. For each targeted file:
   - The first 16 bytes are read and passed to `image::guess_format()`.
   - If the detected format's canonical extensions do not include the file's current extension, the file is renamed on disk (extension swapped, path/date prefix unchanged) and the DB columns `target_path` and `ext` are updated.
   - If the format cannot be detected (RAW, video, already correct), the file is silently skipped.
4. File list reloads from DB.
5. Status bar shows: `fixed N file(s)`, `N file(s) already correct`, or `N error(s) — <msg>`.

---

### Detection scope

Detection relies on the `image` crate's magic-byte reader.  With the current feature set (`jpeg`, `png`, `bmp`, `gif`, `webp`) the following mismatches are caught:

| Actual format | Wrong ext example |
|---------------|-------------------|
| JPEG | `.png`, `.bmp`, … |
| PNG  | `.jpg`, `.jpeg`, … |
| BMP  | `.jpg`, `.png`, … |
| GIF  | `.jpg`, `.png`, … |
| WEBP | `.jpg`, `.png`, … |

RAW formats and video files are not probed and are left unchanged.

---

### Implementation

| Symbol | Location |
|--------|----------|
| `detect_wrong_ext(path, claimed_ext) -> Option<String>` | `src/import.rs` (pub crate) |
| `fix_ext(db_path, target_root, file_id, new_ext)` | `src/db.rs` |
| `App::fix_ext_selected()` | `src/app.rs` |
