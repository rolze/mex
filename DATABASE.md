# mex Database Design

This document is the authoritative reference for the `mex.db` SQLite schema — what it contains, why each element exists, and what was deliberately left out.

## Live database snapshot

| Table      | Rows    | Notes                                          |
|------------|---------|------------------------------------------------|
| media      | 58,404  | 53,873 imported, 4,527 duplicate, 4 trashed    |
| media_tags | 40,946  | avg 0.7 tags/file                              |
| tags       | 68      | unique tags                                    |
| config     | 1       | legacy — `target_root` duplicates config.toml  |
| events     | —       | new; replaces `moved_at`/`scanned_at` on media |

File size: ~56 MB, page size 4096 B, WAL mode.

---

## Schema

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;
PRAGMA cache_size   = -65536;   -- 256 MB; whole DB fits after first load
PRAGMA temp_store   = memory;
PRAGMA foreign_keys = ON;

-- ── tags ──────────────────────────────────────────────────────────────────────
CREATE TABLE tags (
    id   INTEGER PRIMARY KEY,
    name TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    type TEXT    NOT NULL DEFAULT 'event'
) STRICT;

-- ── media ─────────────────────────────────────────────────────────────────────
CREATE TABLE media (
    id               TEXT    PRIMARY KEY,
    source_path      TEXT    NOT NULL UNIQUE,

    -- Bare filename stem, no year-dir prefix, no extension.
    -- Full path = path_stem[0..4] || '/' || path_stem || ext
    -- Year dir always equals path_stem[0..4] — verified across all 58 K rows.
    -- Lexicographic ORDER BY path_stem produces correct collision order without
    -- a Rust helper (e.g. "chisel" < "chisel-2").
    -- caption_slug and derived_slug are computed at runtime via PATH_RE.
    -- NULL for duplicate rows that have no target path yet.
    path_stem        TEXT    UNIQUE,

    partial_hash     TEXT    NOT NULL,  -- first 256 KiB hash; always computed at import

    file_size        INTEGER NOT NULL,
    ext              TEXT    NOT NULL CHECK(ext LIKE '.%'),  -- always dot-prefixed

    -- Final authoritative datetime, second precision: "YYYY-MM-DD HH:MM:SS".
    -- Computed once at execute time (after EXIF is read from the copied file)
    -- using the best-fit priority: filename timestamp > EXIF > source OS mtime > noon UTC.
    -- Acts as the single source of truth for:
    --   • OS mtime stamped on the target file immediately after copy
    --   • mtime repair (:fix-os-time) — no re-derivation needed
    --   • deslugify — uses derived_at[0..10] for YYYY-MM-DD components
    -- NOT NULL: slug-based filenames encode only YYYY-MM; the full date+time is only here.
    derived_at       TEXT    NOT NULL,

    -- Raw source-file date inputs, captured once at import.
    -- NULL = absent in source file. These are inputs to derived_at, never mutated after import.
    orig_exif_date   TEXT,
    orig_xmp_date    TEXT,           -- NULL = no XMP sidecar (replaces has_xmp_sidecar)
    orig_os_date     TEXT,           -- source file OS mtime, captured at scan

    status           TEXT    NOT NULL DEFAULT 'imported'
                             CHECK(status IN ('imported','duplicate','trashed','deleted')),
    missing_on_disk  INTEGER NOT NULL DEFAULT 0,

    -- Denormalized tag cache. Updated atomically inside the same transaction as
    -- any media_tags mutation. Format: CHAR(31)-separated strings (unit separator),
    -- matching the existing serialization already used in the Rust code.
    tags_packed      TEXT    NOT NULL DEFAULT '',
    tag_types_packed TEXT    NOT NULL DEFAULT ''
) STRICT;

-- ── events ────────────────────────────────────────────────────────────────────
-- Append-only lifecycle log. Replaces moved_at / scanned_at columns on media.
-- event_type values: 'imported', 'scanned'
CREATE TABLE events (
    id         INTEGER PRIMARY KEY,
    media_id   TEXT    NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    event_type TEXT    NOT NULL,
    timestamp  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ── media_tags ────────────────────────────────────────────────────────────────
CREATE TABLE media_tags (
    media_id TEXT    NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (media_id, tag_id)
) STRICT;

-- ── indexes ───────────────────────────────────────────────────────────────────
-- Covers exact-match collision checks + LIKE prefix counter detection during import
CREATE INDEX idx_media_path_stem    ON media(path_stem);

-- Covers status != 'deleted' filter on load_files() and trash-count queries
CREATE INDEX idx_media_status       ON media(status);

-- Covers duplicate detection by (file_size, partial_hash) — the only hash needed
CREATE INDEX idx_media_partial_hash ON media(partial_hash);

-- Covers tag-side JOIN (tag_id → media_ids); media_id side covered by PK
CREATE INDEX idx_media_tags_tag     ON media_tags(tag_id);

-- Covers import history query: SELECT MAX(timestamp) ... WHERE event_type = 'imported'
CREATE INDEX idx_events_media       ON events(media_id, event_type);
```

---

## Column rationale

### `path_stem` (replaces `target_path`)

The old `target_path` stored the full relative path including year directory and extension:
`"2022/2022-04-18-stiegenhaus.jpg"`. This caused two problems:

1. **Sorting**: the extension suffix breaks simple lexicographic order for collision variants
   (`"chisel.jpg"` sorts after `"chisel-2.jpg"` because `'.'` < `'-'`). A Rust `path_sort_key()`
   helper was needed to correct this after loading. With `path_stem = "2022-04-18-chisel"`,
   `ORDER BY path_stem` is naturally correct without any post-processing.

2. **Redundancy**: the year directory always equals `path_stem[0..4]` (verified: 0 mismatches
   across all 58 K rows). The extension is stored in `ext`. Full path reconstruction:
   ```
   path_stem[0..4] + "/" + path_stem + ext
   ```

`caption_slug` and `derived_slug` are both computable at runtime from `path_stem` via the
`PATH_RE` pattern — no storage needed.

### `partial_hash` (NOT NULL)

A hash of the first 256 KiB of the file (`PARTIAL_HASH_BYTES`). Always computed at import —
for both successfully imported files and detected duplicates. Used as the fast pre-filter in
deduplication: `(file_size, partial_hash)` pairs are loaded into RAM before the copy loop,
so full-file hash computation is only needed when a size+partial collision is found.

Making this NOT NULL has several benefits:
- Deduplication queries need no `IS NOT NULL` guards
- `ensure_partial_hashes()` backfill path is eliminated
- Duplicate rows carry their hash as a reference point for future re-imports
- A plain index on `partial_hash` covers all rows (no partial index needed)

There is no runtime penalty: the partial hash of every incoming file is computed
unconditionally during the dedup probe (before we know whether it's a duplicate),
so storing it is a zero-cost write of already-available data.
In the current DB, the 4,527 `duplicate` rows have `partial_hash = NULL` only because
the old import code discarded the computed hash instead of writing it.

### `derived_at` (replaces `derived_date`, NOT NULL)

The final authoritative datetime for the file, stored as `"YYYY-MM-DD HH:MM:SS"`.

Computed **once** at execute time (after the file is copied and EXIF is read from the
local destination) using the best-fit priority chain:

1. Full timestamp embedded in the original filename (`filename_secs`)
2. Source OS mtime when its `YYYY-MM` prefix agrees with the derived date
3. EXIF `DateTimeOriginal` (second precision from camera)
4. `YYYY-MM-DD 12:00:00` (noon UTC fallback — avoids ±12 h timezone flip)

**Source of truth after import**: `derived_at` is stamped onto the target file's OS
mtime immediately after copy, and is re-applied by `:fix-os-time` without re-deriving.
No other column needs to be consulted for mtime operations.

**Replaces two responsibilities** held separately in the old schema:
- `derived_date` (day-only date, `YYYY-MM-DD`) — `derived_at[0..10]` gives the same value
- `os_date` (post-fix full datetime written by `:fix-os-time`) — now unified into `derived_at`

`deslugify_batch()` uses `derived_at[0..10]` for `YYYY-MM-DD` components — no change
in its slicing logic.

NOT NULL: slug-based filenames encode only `YYYY-MM`. `deslugify_batch()` needs the full
`DD` component (and now `HH:MM:SS`), which is only available here after EXIF is read.

### `orig_exif_date`, `orig_xmp_date`, `orig_os_date`

Raw source-file date properties, captured once at import and never mutated. The `orig_`
prefix makes their role explicit: immutable inputs to the `derived_at` computation.

`orig_xmp_date IS NOT NULL` serves as the XMP sidecar indicator — the old `has_xmp_sidecar`
boolean was redundant.

### `tags_packed` / `tag_types_packed`

Denormalized tag cache. The `load_files()` query previously joined `media_tags` and `tags`
on every startup (58 K rows × 2 index lookups to assemble 40 K tag pairs). These columns
eliminate that JOIN entirely — the packed strings are pre-assembled and read in a single
table scan.

The format — `CHAR(31)`-separated (unit separator, `\x1f`) — matches the serialization
already used in the Rust codebase.

**Mutation rule**: whenever `media_tags` is modified for a row, update `tags_packed` and
`tag_types_packed` for that row in the same transaction:

```sql
UPDATE media SET
    tags_packed      = (SELECT COALESCE(GROUP_CONCAT(t.name, CHAR(31)), '')
                        FROM media_tags mt JOIN tags t ON t.id = mt.tag_id
                        WHERE mt.media_id = media.id),
    tag_types_packed = (SELECT COALESCE(GROUP_CONCAT(t.type, CHAR(31)), '')
                        FROM media_tags mt JOIN tags t ON t.id = mt.tag_id
                        WHERE mt.media_id = media.id)
WHERE id = ?;
```

### `status` values

The old value `'moved'` meant "file is in the archive with a valid target path" — named after
the original workflow where import physically moved files from source to target. The source is
no longer deleted on import (files are copied), so `'moved'` is misleading.

Renamed to `'imported'`: file was copied into the archive and has a valid `path_stem`.

Valid values:
- `'imported'` — active, in archive, normal state (was `'moved'`)
- `'duplicate'` — detected as a hash duplicate; has no `path_stem`
- `'trashed'` — soft-deleted; retains `path_stem` for potential restore
- `'deleted'` — physically removed from filesystem

The in-memory `ImportStatus::Pending` / `ImportStatus::UnknownDate` from the Rust import
pipeline are never written to the DB — files are inserted directly as `'imported'` on success.

### `events` (replaces `moved_at` / `scanned_at`)

`media` is an entity record — what a file *is*. `moved_at` and `scanned_at` are temporal
events — what *happened* to it. Moving them to an append-only `events` table keeps `media`
clean and allows multiple events of the same type per file.

The import history query changes from `MAX(m.moved_at)` to:
```sql
SELECT t.name, MAX(e.timestamp)
FROM events e
JOIN media m  ON m.id = e.media_id
JOIN media_tags mt ON mt.media_id = m.id
JOIN tags t   ON t.id = mt.tag_id
WHERE e.event_type = 'imported'
GROUP BY t.name
ORDER BY MAX(e.timestamp) DESC
```

---

## Dropped columns

| Column | Reason |
|--------|--------|
| `content_hash` | Write-only in DB — never SELECTed. Deduplication uses `(file_size, partial_hash)` exclusively. Dropping it also removes the full-file SHA-256 computation from `stream_copy_and_hash`, saving CPU per import. |
| `derived_date` | Renamed to `derived_at` and promoted to full `YYYY-MM-DD HH:MM:SS` precision. |
| `os_date` | Post-fix full datetime written by `:fix-os-time`. Unified into `derived_at` — no separate column needed. |
| `derived_slug` | Legacy (22 K stale rows). Derivable at runtime from `path_stem` via `PATH_RE`. Never read from DB in current code. |
| `caption_slug` | Derivable at runtime from `path_stem` via `PATH_RE`. Only ever written, never selectively queried. |
| `date_source` | Write-only in DB. Its value is read from the in-memory `ImportEntry` struct for the import preview UI — never SELECTed back from the database. |
| `slug_source` | Same as `date_source`: written at import, never read back from DB. |
| `counter` | A cached `MAX(counter)` result. With `idx_media_path_stem`, counter detection uses a `LIKE` prefix scan + parse the trailing digit in Rust. ~50–200 matching rows per day-prefix, negligible. |
| `has_xmp_sidecar` | `orig_xmp_date IS NOT NULL` is the exact equivalent. Storing a separate boolean is redundant. |
| `scanned_at` | Moved to `events(event_type='scanned')`. Was write-only (agent tooling). |
| `moved_at` | Moved to `events(event_type='imported')`. Was actively queried in import history. |

## Dropped tables

| Table | Rows | Reason |
|-------|------|--------|
| `actions` | 52,915 | Pre-mex agent audit log. Not queried or written by any mex code path. Pure legacy. |
| `scans` | 44 | Pre-mex import diagnostics. Not used by mex. |
| `config` | 1 | `target_root` is already the authoritative value in `~/.config/mex/config.toml` (managed by `config.rs`). No code reads `target_root` from the DB. Redundant duplicate. |

---

## Performance impact

### `load_files()` — startup query

**Before:**
```sql
SELECT m.id, m.target_path, ...
  GROUP_CONCAT(t.name || CHAR(30) || t.type, CHAR(31))
FROM media m
LEFT JOIN media_tags mt ON mt.media_id = m.id
LEFT JOIN tags t ON t.id = mt.tag_id
WHERE m.target_path IS NOT NULL AND m.status != 'deleted'
GROUP BY m.id
```
→ 58 K rows, 2 index lookups each to assemble 40 K tag pairs, `GROUP BY` pass.  
→ Rust `path_sort_key()` called on every row after load for correct sort order.

**After:**
```sql
SELECT id, path_stem, ext, derived_at,
       tags_packed, tag_types_packed,
       orig_os_date, source_path, status, missing_on_disk
FROM media
WHERE path_stem IS NOT NULL AND status != 'deleted'
ORDER BY path_stem
```
→ Single table scan. No JOIN, no GROUP BY, no GROUP_CONCAT. SQL handles sort.  
→ `path_sort_key()` eliminated. `caption_slug` computed from `path_stem` in Rust post-load.

### Import / rename — collision queries

**Before:** no index on `target_path` — full 58 K-row scan for every collision check and counter
detection query. Called multiple times per imported file.

**After:** `idx_media_path_stem` — exact-match and `LIKE` prefix queries become single B-tree
lookups. Counter detection reads ~50–200 rows, parses trailing digit in Rust.

### Sort order

**Before:** `ORDER BY target_path` puts `.jpg` before `-2.jpg` (wrong); post-sort correction
in Rust via `path_sort_key()` required.

**After:** `ORDER BY path_stem` is naturally correct — no Rust helper.

### Cache

**Before:** `PRAGMA cache_size = -2000` = 8 MB. The 56 MB DB page-faults on every cold load.

**After:** `PRAGMA cache_size = -65536` = 256 MB. Entire DB fits in RAM after the first load;
subsequent queries are fully in-memory.

### DB size

Dropping `actions` (52 K rows) and `scans` (44 rows) reduces file size by roughly 20–30%.
The `media` table shrinks by 5 columns (derived_slug, caption_slug, date_source, slug_source,
counter) — roughly 8–12 % row size reduction.
