## UC-08 · Smart import of new files

**Actor:** User connects a new drive / phone and integrates all new media files  
**Goal:** new media files are slug-normalised, dated, deduplicated, and copied to the target tree; each import session is tagged

---

### Command

```
:import <path>
```

`<path>` must be an existing directory. It is the full trimmed remainder of the command
line after `import `, so paths containing `:` (e.g. MTP mount points) and spaces work
without quoting. The target root is taken from `config.target_dir`.

**Path autosuggestion:** on startup mex reads the `import-*` (type `mex`) tags from the
DB and reconstructs the source root for each past import session (longest common path
prefix of all `source_path` values in that tag group). While typing `:import `, Up/Down
cycles through these directories in recency order (most recent first); Tab completes to
the highlighted suggestion. A dim inline suffix previews the completion ahead of the
cursor. New/unknown paths are always accepted.

**Availability hint:** after a 400 ms debounce following the last keystroke, mex checks
whether the typed path exists on the filesystem and appends a coloured indicator after
the cursor:
- `✓` (green) — path exists and is a directory
- `✗` (red) — path does not exist
- `✗ not a dir` (red) — path exists but is not a directory

---

### Flow

1. **Scan** — runs in a background thread:
   - Walks `<path>` recursively; skips quality-variant dirs (`small`, `web`, `thumbnail`, …), junk folders (DCIM, `\d{3}CANON`, MISC), and always-skip file types (`.txt`, `.db`, `.ini`, …).
   - For each media file: reads only metadata (size + mtime from walkdir cached stat — no file open at all) → derive date → derive slug. SHA-256 hashing, EXIF, and dedup are deferred to the Execute phase. Progress is reported per file (count + current filename shown in overlay).
   - Date derivation priority: filename patterns → folder path date → OS mtime fallback. (EXIF is deferred; XMP sidecars are not expected from Android phones and are skipped.)
   - Filename date patterns: `yyyyMMdd`, `yyyy-MM-dd`, `DD-MM-YY_HHMM` mobile, `YY-MM-DD` with separators (Picsart), 13-digit unix-ms anywhere in name (phoneImageCapture, Revolut), 8-digit prefix after `snap` prefix, `yyyy-MM` year-month.
   - UUID file stems (8-4-4-4-12 hex) bypass all filename date/slug parsing; OS mtime is used instead.
   - Slug derivation: ancestor-folder walk up to 4 tokens, transliterating umlauts (ä→ae, ö→oe, ü→ue, ß→ss), stripping junk words, camera codes, UUID fragments, hex garbage tokens, and sequence-number tokens (e.g. `WA0004`). Junk words include `"camera"` (Android `DCIM/Camera/` folder).
   - Sends `ImportMsg::ScanDone(Vec<ImportEntry>)` on completion or `ImportMsg::ScanError(msg)` on failure.
   - Simultaneously applies folder-mtime consensus (normalise year-month within each source folder using EXIF majority).
   - **`[Esc]` aborts** — the UI returns to idle immediately; the background thread finishes silently and its result is discarded.

2. **Preview screen** — full-screen TUI dry run:
   - Before preview is shown, `deduplicate_within_batch()` is called: entries are grouped by `(file_size, partial_hash)` (256 KiB probe); within each group the file that sorts first by `(date, slug, source_path)` survives as `Pending`; the rest are marked `Duplicate`. This ensures counter slots are not wasted on files that will be skipped later.
   - Stats bar: total files found / pending / duplicate / skipped / unknown-date counts.
   - Scrollable list: `source_basename → target_path | date_src | slug_src | status` for every entry.
   - List title shows scroll position: `N ready  end/total  ↑↓/PgDn/PgUp`.
   - Keys: `↑`/`↓` (one line), `PgDn`/`PgUp` (one page); scroll is clamped so the last entry stays at the bottom.
   - Footer: `[y/Enter] confirm  [Esc] cancel`.

3. **Execute** — on confirmation, runs in a background thread:
   - `assign_counters()` — for each unique `YYYY/date-slug` prefix: query `MAX(counter)` from DB (`status IN ('moved','trashed','deleted')`) AND scan existing filesystem dir; take max+1 as 4-digit counter.
     - **Caption-only files** (no `derived_slug`, has `caption_slug`): the first file for a given caption gets a plain path with no counter (`yyyy/yyyy-mm-dd-{caption}.ext`). A collision (same caption already in the batch, in the DB, or on disk) triggers a **per-caption collision counter** keyed by `yyyy-mm-dd-{caption}`, initialised from `MAX(counter)` of matching `yyyy/yyyy-mm-dd-{caption}-%` paths in DB/FS; this counter starts at **2** (the plain path is implicitly "version 1") and increments independently for each different caption. Collision paths are stored as `yyyy/yyyy-mm-dd-{caption}-{N}.ext` with `counter = N` in the DB. Plain caption paths store `counter = NULL` in the DB.
   - Load dedup set from DB: `(file_size, partial_hash)` pairs for all rows with `status IN ('moved','trashed','deleted')` that have a `partial_hash`. This ensures that trashed and permanently-deleted files are never reimported (see [[UC-11.md]]).
   - For each `pending` entry:
     - **Partial-hash probe** — read first 256 KiB from source, compute SHA-256. If `(file_size, probe_hash)` is in the DB dedup set → skip immediately. No file copy, no further source I/O.
     - If not a duplicate → stream-copy source → dest while computing full SHA-256 in one pass → store both `partial_hash` and `content_hash` in DB.
     - **mtime preservation** — after each successful copy, the destination file's OS mtime is set to the best available authoritative timestamp (non-fatal; silently skipped on filesystems that reject mtime updates such as exFAT/WSL2):
       1. **Filename full timestamp** — if the filename encodes a complete datetime (13-digit Unix-ms, `yyyyMMdd_HHmmss`, snap+HHMM, Nokia `DD-MM-YY_HHMM`, Picsart `YY-MM-DD_HH-MM-SS`).
       2. **Source OS mtime** — if the source mtime's `YYYY-MM` prefix matches `derived_date` (preserves actual day+time within a year-month-precision filename, e.g. a `2023-07-…` target keeps the `2023-07-15 14:30` source mtime).
       3. **Derived date at noon UTC** — fallback when source mtime's month disagrees; noon avoids ±12 h timezone flips.
     - Within-batch dedup for any stragglers also uses `(file_size, partial_hash)` so any duplicates that slipped through (different file sizes from previous scan inconsistencies) are caught without copying.
   - Full-screen progress overlay shows spinner, `done / total (%)`, ASCII progress bar `████░░░░`, current filename being copied, and `[Esc] abort`.
   - **`[Esc]` aborts** — cancellation is checked at the start of each file (never mid-copy), so the current file always completes cleanly. Files already imported this session remain on disk and in the DB. The import tag is assigned to those files before stopping. The UI returns to idle immediately; the background thread finishes the current file and exits.
   - DB `INSERT OR REPLACE INTO media` with all columns including `partial_hash`, `status='moved'`.
   - After all files (or on abort): create tag `import-YY-MM-DD` (type `mex`) for the first import of that day; if a tag with that name already exists, use `import-YY-MM-DD_2`, `_3`, … to give each session a unique tag. Assign the tag to all newly imported media rows in a single atomic transaction.
   - Sends `ImportMsg::CopyProgress { done, total, current_file }` updates and `ImportMsg::CopyDone(ImportSummary)` on finish.

4. **Done** — media list reloads; status bar shows summary (`N copied, M duplicates, K skipped`).

---

### Data types (src/import.rs)

| Type | Values |
|------|--------|
| `ImportStatus` | `Pending`, `Duplicate`, `Skipped`, `UnknownDate` |
| `DateSource` | `Exif`, `Filename`, `Folder`, `OsDate`, `Unknown` |
| `DatePrecision` | `Full`, `YearMonth`, `YearOnly` |
| `ImportMsg` | `ScanDone`, `ScanError`, `CopyProgress`, `CopyDone`, `CopyError` |
| `ImportState` (app) | `Idle`, `Scanning`, `Preview(Vec<ImportEntry>)`, `Copying{done,total}`, `Done(String)` |

`ImportEntry.wrong_ext` — not populated during scan (magic-byte check requires a file open). Reserved for future use.

---

### DB interaction

- `partial_hash` — SHA-256 of the first 256 KiB of each file, stored in `media`. Used as the fast duplicate probe key during import; computed at copy time for new files and backfilled for existing files by `migrate-partial-hashes`.
- Dedup set loaded at execute time: `HashMap<(file_size, partial_hash), target_path>` from all `status='moved'` rows with non-NULL `partial_hash`.
- `INSERT OR REPLACE INTO media` — all existing columns plus `partial_hash`; no other schema changes.
- Import tag: `import-YY-MM-DD` with type `mex` (2-digit year, e.g. `import-25-01-15`); a second import on the same day gets `import-25-01-15_2`, a third `_3`, etc.
- Background DB workers (`:import`, `:deslugify`, `:fix-os-time`) open dedicated SQLite connections with a 5 s `busy_timeout` to reduce transient lock failures while the UI keeps its own connection open.

---

### Schema migration

Run once after upgrading from a pre-partial-hash DB:

```
cargo run --bin migrate-partial-hashes -- [--db <path>] [--root <path>]
```

Defaults: `--db .mex.db`, `--root` from `~/.config/mex/config.toml`.
Idempotent — safe to interrupt and re-run; already-migrated rows are skipped.

---

### Slug management commands

`:deslugify` and `:slugify <slug>` — see [[UC-16-slugify.md]].

---

### Repair command: `:fix-os-time`

```
:fix-os-time
```

Operates on the selected file(s) (or cursor file). Re-applies the same OS mtime
logic as the import execute phase to files whose mtime was not set correctly at
import time.

**Per-file behaviour:**

1. Read `target_path`, `derived_date`, and `source_path` from the DB.
2. Re-derive `filename_secs` from the original source filename (same patterns as
   import scan: 13-digit Unix-ms, `yyyyMMdd_HHmmss`, snap prefix, Nokia, Picsart, …).
3. Select the best timestamp (same priority as import):
   - P1: full timestamp from original filename (`filename_secs`).
   - P2: `derived_date` at noon UTC (source mtime unavailable for repair).
4. Call `filetime::set_file_mtime` on the target file. Non-fatal if the filesystem
   rejects mtime updates (e.g. exFAT/WSL2).
5. Skip silently when `derived_date` is absent or the target file is missing on disk.

**Status messages:**

| Outcome | Message |
|---------|---------|
| All updated | `fix-os-time: updated N file(s)` |
| Some skipped | `fix-os-time: updated N file(s), M skipped (no date)` |
| All skipped | `fix-os-time: M file(s) skipped (no date)` |
| Error(s) | `fix-os-time: N error(s) — <first error>` |

The command runs in a background thread with a full-screen progress overlay while
in progress; all key input is blocked until the thread finishes.