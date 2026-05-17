use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};

#[derive(Clone, Debug)]
pub struct MediaFile {
    pub id: String,
    pub target_path: String,
    pub derived_date: String,
    pub ext: String,
    pub tags: Vec<String>,
    /// Tag types parallel to `tags` (same indices). E.g. "event", "person", "album", "camera".
    pub tag_types: Vec<String>,
    pub derived_slug: String,
    pub caption_slug: String,
    /// Full OS datetime stored in the DB (e.g. `"2022-04-18 14:30:00"`). Empty if not set.
    pub os_date: String,
    /// Original filename (basename of `source_path`) before import. Empty if not set.
    pub orig_filename: String,
    /// DB status value: `"moved"` (normal), `"trashed"` (soft-deleted), `"deleted"` (FS removed).
    pub status: String,
    /// True when the physical file was found absent under `target_root` the last time the
    /// preview was opened for this file.  Persisted in the DB so the indicator survives
    /// across sessions.  Cleared automatically when the file is found again.
    pub missing_on_disk: bool,
}

pub fn load_files(db_path: &str) -> Result<Vec<MediaFile>> {
    let conn = Connection::open(db_path)?;

    let sql = "SELECT m.id, m.target_path, COALESCE(m.derived_date,''), m.ext,
                      COALESCE(GROUP_CONCAT(t.name || CHAR(30) || t.type, CHAR(31)),''),
                      COALESCE(m.derived_slug,''), COALESCE(m.caption_slug,''),
                      COALESCE(m.os_date,''), COALESCE(m.source_path,''),
                      COALESCE(m.status,'moved'),
                      COALESCE(m.missing_on_disk,0)
               FROM media m
               LEFT JOIN media_tags mt ON mt.media_id = m.id
               LEFT JOIN tags t ON t.id = mt.tag_id
               WHERE m.target_path IS NOT NULL
               GROUP BY m.id
               ORDER BY m.target_path";

    let mut stmt = conn.prepare(sql)?;

    let rows = stmt.query_map([], |row| {
        let tags_str: String = row.get(4)?;
        let (tags, tag_types) = if tags_str.is_empty() {
            (vec![], vec![])
        } else {
            tags_str.split('\x1f')
                .map(|pair| {
                    if let Some(sep) = pair.find('\x1e') {
                        (pair[..sep].to_string(), pair[sep + 1..].to_string())
                    } else {
                        (pair.to_string(), String::new())
                    }
                })
                .unzip()
        };
        let source_path: String = row.get(8)?;
        let orig_filename = std::path::Path::new(&source_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(MediaFile {
            id: row.get(0)?,
            target_path: row.get(1)?,
            derived_date: row.get(2)?,
            ext: row.get(3)?,
            tags,
            tag_types,
            derived_slug: row.get(5)?,
            caption_slug: row.get(6)?,
            os_date: row.get(7)?,
            orig_filename,
            status: row.get(9)?,
            missing_on_disk: row.get::<_, i64>(10)? != 0,
        })
    })?;

    let mut files = Vec::new();
    for r in rows {
        files.push(r?);
    }
    Ok(files)
}

/// Derive the folder portion from a target_path like "2022/2022-04-18-foo.jpeg"
pub fn folder_of(path: &str) -> &str {
    if let Some(pos) = path.rfind('/') {
        &path[..pos]
    } else {
        "."
    }
}

/// Initialise the DB schema on a fresh database and migrate existing ones.
///
/// Safe to call every startup — all statements use `CREATE TABLE IF NOT EXISTS`
/// and `ensure_schema_v1` is a no-op when already at the current version.
pub fn init_db(db_path: &str) -> Result<()> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS media (
            id           TEXT PRIMARY KEY,
            source_path  TEXT,
            target_path  TEXT,
            content_hash TEXT,
            file_size    INTEGER,
            ext          TEXT,
            exif_date    TEXT,
            derived_date TEXT,
            date_source  TEXT,
            derived_slug TEXT,
            caption_slug TEXT,
            slug_source  TEXT,
            counter      INTEGER,
            status       TEXT,
            os_date      TEXT,
            scanned_at   TEXT,
            moved_at     TEXT
        );
        CREATE TABLE IF NOT EXISTS tags (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT,
            type TEXT DEFAULT 'event'
        );
        CREATE TABLE IF NOT EXISTS media_tags (
            media_id TEXT,
            tag_id   INTEGER,
            PRIMARY KEY (media_id, tag_id)
        );",
    )?;
    ensure_schema_v1(&conn)?;
    ensure_schema_v2(&conn)?;
    Ok(())
}

/// Migrate the DB schema to version 1 (adds `partial_hash` column).
///
/// Safe to call on every connection open — it is a no-op when the DB is
/// already at the current version.  Uses `PRAGMA user_version` as the schema
/// version counter (0 = original, 1 = adds `partial_hash`).
pub fn ensure_schema_v1(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(
            "ALTER TABLE media ADD COLUMN partial_hash TEXT;
             PRAGMA user_version = 1;",
        )?;
    }
    Ok(())
}

/// Migrate the DB schema to version 2 (adds `missing_on_disk` column).
///
/// Safe to call on every connection open — it is a no-op when the DB is
/// already at the current version.
pub fn ensure_schema_v2(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 2 {
        conn.execute_batch(
            "ALTER TABLE media ADD COLUMN missing_on_disk INTEGER NOT NULL DEFAULT 0;
             PRAGMA user_version = 2;",
        )?;
    }
    Ok(())
}

/// Set or clear the `missing_on_disk` flag for a single media file.
///
/// Called lazily when the terminal preview is opened for a file and mex checks
/// whether the physical file exists under `target_root`.
pub fn set_missing_on_disk(db_path: &str, media_id: &str, missing: bool) -> Result<()> {
    let conn = Connection::open(db_path)?;
    conn.execute(
        "UPDATE media SET missing_on_disk = ?1 WHERE id = ?2",
        rusqlite::params![missing as i64, media_id],
    )?;
    Ok(())
}

/// Attach a tag (name + type) to each media file in `media_ids`.
///
/// `tag_type`:
/// - `Some(ty)` — explicit type: reuse if existing type matches (case-insensitive),
///   error if it differs, create with `ty` if new.
/// - `None` — type omitted: reuse the tag regardless of its existing type,
///   or create it as `"event"` if it does not yet exist.
///
/// Files that already carry the tag are silently skipped (INSERT OR IGNORE).
///
/// Returns the effective tag type that was used (for status messages).
pub fn assign_tag(
    db_path: &str,
    media_ids: &[String],
    tag_name: &str,
    tag_type: Option<&str>,
) -> Result<String> {
    let mut conn = Connection::open(db_path)?;

    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, type FROM tags WHERE name = ?1 COLLATE NOCASE",
            rusqlite::params![tag_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let (tag_id, effective_type) = match existing {
        Some((id, existing_type)) => {
            if let Some(requested) = tag_type {
                if !existing_type.eq_ignore_ascii_case(requested) {
                    return Err(anyhow::anyhow!(
                        "tag '{}' already exists as type '{}'",
                        tag_name,
                        existing_type
                    ));
                }
            }
            (id, existing_type)
        }
        None => {
            let ty = tag_type.unwrap_or("event");
            conn.execute(
                "INSERT INTO tags (name, type) VALUES (?1, ?2)",
                rusqlite::params![tag_name, ty],
            )?;
            (conn.last_insert_rowid(), ty.to_string())
        }
    };

    if !media_ids.is_empty() {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached("INSERT OR IGNORE INTO media_tags (media_id, tag_id) VALUES (?1, ?2)")?;
            for media_id in media_ids {
                stmt.execute(rusqlite::params![media_id, tag_id])?;
            }
        }
        tx.commit()?;
    }

    Ok(effective_type)
}

/// Remove tags from media files.
///
/// - `tag_names` empty → remove **all** tags from every file in `media_ids`.
/// - `tag_names` non-empty → remove only the named tags (case-insensitive).
///
/// Returns the number of `media_tags` rows deleted.
pub fn remove_tags(
    db_path: &str,
    media_ids: &[String],
    tag_names: &[String],
) -> Result<usize> {
    if media_ids.is_empty() {
        return Ok(0);
    }

    let conn = Connection::open(db_path)?;

    let id_ph: String = std::iter::repeat("?").take(media_ids.len()).collect::<Vec<_>>().join(",");

    let removed = if tag_names.is_empty() {
        conn.execute(
            &format!("DELETE FROM media_tags WHERE media_id IN ({id_ph})"),
            rusqlite::params_from_iter(media_ids),
        )?
    } else {
        let name_ph: String = std::iter::repeat("?").take(tag_names.len()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "DELETE FROM media_tags WHERE media_id IN ({id_ph}) \
             AND tag_id IN (SELECT id FROM tags WHERE name IN ({name_ph}) COLLATE NOCASE)"
        );
        let params: Vec<&String> = media_ids.iter().chain(tag_names.iter()).collect();
        conn.execute(&sql, rusqlite::params_from_iter(params))?
    };

    Ok(removed)
}

/// Replace the date prefix in a MEX basename with `new_date` (`yyyy-mm-dd`).
///
/// Handles two filename formats:
/// - Day format:  `yyyy-MM-DD-rest` → `{new_date}-rest`
/// - Slug format: `yyyy-MM-<non-digit>…` → `{new_yyyy}-{new_MM}-rest-tail`
///
/// If neither pattern matches, returns the basename unchanged.
pub fn rename_file_date(basename: &str, new_date: &str) -> String {
    // Day format: starts with exactly yyyy-MM-DD followed by end or '-'
    // e.g. "2022-04-18-0001.jpg" or "2022-04-18.jpg"
    let day_re_match = basename.len() >= 10
        && basename.as_bytes()[4] == b'-'
        && basename.as_bytes()[7] == b'-'
        && basename[..4].chars().all(|c| c.is_ascii_digit())
        && basename[5..7].chars().all(|c| c.is_ascii_digit())
        && basename[8..10].chars().all(|c| c.is_ascii_digit())
        && (basename.len() == 10 || basename.as_bytes()[10] == b'.' || basename.as_bytes()[10] == b'-');

    if day_re_match {
        // Replace the first 10 chars (yyyy-MM-DD) with new_date
        return format!("{}{}", new_date, &basename[10..]);
    }

    // Slug format: starts with yyyy-MM-<non-digit>
    // e.g. "2022-04-festival-0001.jpg"
    let slug_match = basename.len() >= 8
        && basename.as_bytes()[4] == b'-'
        && basename[..4].chars().all(|c| c.is_ascii_digit())
        && basename[5..7].chars().all(|c| c.is_ascii_digit())
        && basename.as_bytes()[7] == b'-'
        && basename.len() > 8
        && !basename.as_bytes()[8].is_ascii_digit();

    if slug_match {
        // Replace only yyyy-MM (first 7 chars) with new year-month from new_date
        let new_ym = &new_date[..7]; // "yyyy-mm"
        return format!("{}{}", new_ym, &basename[7..]);
    }

    basename.to_string()
}

/// Fix the date of a single media file: rename on disk, update OS mtime, update DB.
///
/// `new_date` must be in `yyyy-mm-dd` format.
///
/// The operation is atomic with respect to stale data: a **pre-flight mtime
/// check** is performed before any mutation. If the filesystem does not permit
/// `set_file_mtime` (e.g. exFAT via WSL2), the function returns an error
/// immediately and nothing is changed on disk or in the DB.
pub fn fix_date(
    db_path: &str,
    target_root: &str,
    file_id: &str,
    new_date: &str,
) -> Result<()> {
    use filetime::{set_file_mtime, FileTime};
    use std::path::Path;

    let conn = Connection::open(db_path)?;

    // Fetch current target_path and stored os_date (as fallback for time component).
    let (target_path, old_os_date): (String, Option<String>) = conn.query_row(
        "SELECT target_path, os_date FROM media WHERE id = ?1",
        [file_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // Split into folder prefix and basename.
    let (old_folder, basename) = if let Some(pos) = target_path.rfind('/') {
        (&target_path[..pos], &target_path[pos + 1..])
    } else {
        ("", target_path.as_str())
    };

    let new_basename = rename_file_date(basename, new_date);
    let new_year = &new_date[..4];
    // Keep the same sub-folder structure (year folder).
    let new_folder = if old_folder.is_empty() { new_year.to_string() } else { new_year.to_string() };
    let new_target_path = if new_folder.is_empty() {
        new_basename.clone()
    } else {
        format!("{}/{}", new_folder, new_basename)
    };

    let old_abs = Path::new(target_root).join(&target_path);
    let new_abs = Path::new(target_root).join(&new_target_path);

    // Preserve the hh:mm:ss from the file's current mtime, or fall back to
    // the stored os_date, or default to 00:00:00.
    let preserved_hms = read_file_hms(&old_abs)
        .or_else(|| extract_hms_from_str(old_os_date.as_deref().unwrap_or("")))
        .unwrap_or((0, 0, 0));

    // ── Pre-flight mtime check ────────────────────────────────────────────────
    // Before touching anything, verify that the filesystem allows mtime updates.
    // We use the file's *current* mtime as the value (true no-op) so the check
    // is side-effect-free. If this fails (e.g. EPERM on exFAT/WSL2) we abort
    // now — disk and DB are both still intact.
    if old_abs.exists() {
        let current_meta = std::fs::metadata(&old_abs)?;
        let current_mtime = FileTime::from_last_modification_time(&current_meta);
        set_file_mtime(&old_abs, current_mtime).map_err(|e| {
            anyhow::anyhow!(
                "mtime update not supported on this filesystem (exFAT/WSL2?): {e}"
            )
        })?;
    }

    // Create the year directory if needed.
    if let Some(parent) = new_abs.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Rename file on disk (only if paths differ and old file exists).
    let renamed = if old_abs != new_abs && old_abs.exists() {
        std::fs::rename(&old_abs, &new_abs)?;
        true
    } else {
        false
    };

    // Build new datetime string: new date + preserved time.
    let (h, m, s) = preserved_hms;
    let new_os_date = format!("{new_date} {:02}:{:02}:{:02}", h, m, s);

    // Set OS mtime with the new date + preserved time component.
    // If this fails after a rename, revert the rename to avoid leaving the
    // filesystem in an inconsistent state, then propagate the error.
    let path_for_mtime = if new_abs.exists() { &new_abs } else { &old_abs };
    if path_for_mtime.exists() {
        let mtime = datetime_to_filetime(new_date, h, m, s);
        if let Err(e) = set_file_mtime(path_for_mtime, mtime) {
            if renamed {
                // Best-effort revert; ignore secondary errors.
                let _ = std::fs::rename(&new_abs, &old_abs);
            }
            return Err(anyhow::anyhow!(
                "mtime update not supported on this filesystem (exFAT/WSL2?): {e}"
            ));
        }
    }

    // Update DB: os_date keeps full datetime; derived_date is date-only.
    // DB is always written last so a failure above never leaves stale data.
    conn.execute(
        "UPDATE media SET os_date = ?1, derived_date = ?2, target_path = ?3 WHERE id = ?4",
        rusqlite::params![new_os_date, new_date, new_target_path, file_id],
    )?;

    Ok(())
}

/// Read the hh:mm:ss components from a file's mtime (local time).
/// Returns `None` if the file doesn't exist or metadata is unavailable.
fn read_file_hms(path: &std::path::Path) -> Option<(u8, u8, u8)> {
    use std::time::UNIX_EPOCH;
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let secs_since_epoch = mtime.duration_since(UNIX_EPOCH).ok()?.as_secs();
    // Seconds within the UTC day.
    let secs_in_day = (secs_since_epoch % 86400) as u32;
    let h = (secs_in_day / 3600) as u8;
    let m = ((secs_in_day % 3600) / 60) as u8;
    let s = (secs_in_day % 60) as u8;
    Some((h, m, s))
}

/// Extract hh:mm:ss from a stored datetime string like `"2022-04-18 14:30:00"`.
/// Returns `None` if the string is too short or malformed.
fn extract_hms_from_str(s: &str) -> Option<(u8, u8, u8)> {
    // Expect at least "yyyy-mm-dd HH:MM:SS" (19 chars).
    if s.len() < 19 { return None; }
    let time_part = &s[11..19]; // "HH:MM:SS"
    let h: u8 = time_part[0..2].parse().ok()?;
    let m: u8 = time_part[3..5].parse().ok()?;
    let sec: u8 = time_part[6..8].parse().ok()?;
    Some((h, m, sec))
}

/// Build a `FileTime` from a date + explicit hh:mm:ss (all UTC).
fn datetime_to_filetime(date: &str, h: u8, m: u8, s: u8) -> filetime::FileTime {
    let year: i64 = date[0..4].parse().unwrap_or(1970);
    let month: i64 = date[5..7].parse().unwrap_or(1);
    let day: i64 = date[8..10].parse().unwrap_or(1);
    let days = days_from_epoch(year, month, day);
    let secs = days * 86400 + h as i64 * 3600 + m as i64 * 60 + s as i64;
    filetime::FileTime::from_unix_time(secs, 0)
}

/// Compute days since Unix epoch for a given date (proleptic Gregorian calendar).
fn days_from_epoch(year: i64, month: i64, day: i64) -> i64 {
    // Use the civil_from_days / days_from_civil algorithm (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let m = month;
    let d = day;
    let era: i64 = if y >= 0 { y } else { y - 399 } / 400;
    let yoe: i64 = y - era * 400;
    let doy: i64 = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe: i64 = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Fix the extension of a single media file: rename on disk, update DB.
///
/// `new_ext` must be the canonical extension detected from the file's magic bytes
/// (e.g. `"jpg"`).  The caller is responsible for detection; this function only
/// does the rename + DB update.
pub fn fix_ext(db_path: &str, target_root: &str, file_id: &str, new_ext: &str) -> Result<()> {
    use std::path::Path;

    let conn = Connection::open(db_path)?;

    let target_path: String = conn.query_row(
        "SELECT target_path FROM media WHERE id = ?1",
        [file_id],
        |row| row.get(0),
    )?;

    let old_abs = Path::new(target_root).join(&target_path);
    let stem = old_abs
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("cannot determine stem for {}", old_abs.display()))?;

    let new_basename = format!("{stem}.{new_ext}");
    let new_target_path = if let Some(pos) = target_path.rfind('/') {
        format!("{}/{}", &target_path[..pos], new_basename)
    } else {
        new_basename
    };
    let new_abs = Path::new(target_root).join(&new_target_path);

    if old_abs == new_abs {
        return Ok(());
    }
    if !old_abs.exists() {
        anyhow::bail!("file not found: {}", old_abs.display());
    }
    std::fs::rename(&old_abs, &new_abs)?;

    conn.execute(
        "UPDATE media SET target_path = ?1, ext = ?2 WHERE id = ?3",
        rusqlite::params![new_target_path, new_ext, file_id],
    )?;

    Ok(())
}

/// Per-file result produced by [`remove_slug_batch`].
#[derive(Debug)]
pub struct RemoveSlugBatchStats {
    pub fixed: usize,
    pub skipped: usize,
    /// Per-file errors: `(file_id, message)`.
    pub errors: Vec<(String, String)>,
}

/// Remove bad slugs from a batch of media files and assign fresh day-precision counters.
///
/// All counter values for the batch are pre-computed **before** any file is renamed, so
/// selecting every file from a given day correctly restarts the counter at 0001.
///
/// Counter logic (mirrors `assign_counters` in import):
/// - For each unique `yyyy-mm-dd` day prefix in the batch, query `MAX(counter)` from the
///   DB and scan the filesystem, **excluding every file that is part of the current
///   batch** from both queries.  This ensures idempotent re-runs: files that were
///   repaired with wrong counters by a previous run are re-numbered from scratch.
/// - Counters are then assigned sequentially (files sorted by date, then current path)
///   starting at `max + 1`.
///
/// Caption rule (mirrors import): when `caption_slug` is non-empty, the counter-free
/// form `yyyy/yyyy-mm-dd-{caption}.ext` is preferred; a counter suffix is only added on
/// collision with a non-batch path.
///
/// Files whose path and slug are already correct are counted as `skipped` (no-op).
/// Per-file errors (collision, FS failure) are collected in `stats.errors`; the rest of
/// the batch continues.
///
/// `on_progress(index, file_id)` is called before each file is processed.
pub fn remove_slug_batch(
    db_path: &str,
    target_root: &str,
    file_ids: &[String],
    on_progress: impl Fn(usize, &str),
) -> Result<RemoveSlugBatchStats> {
    use std::{
        collections::{HashMap, HashSet},
        path::Path,
    };

    if file_ids.is_empty() {
        return Ok(RemoveSlugBatchStats { fixed: 0, skipped: 0, errors: Vec::new() });
    }

    let conn = Connection::open(db_path)?;

    // ── 1. Load all records ────────────────────────────────────────────────
    struct Rec {
        id: String,
        target_path: String,
        derived_date: String,
        derived_slug: String,
        caption_slug: String,
        ext: String,
        /// OS mtime stored at import time, e.g. `"2025-11-30 14:30:00"`.  Empty if unset.
        os_date: String,
        /// Basename of the original source path (pre-import filename).  Empty if unset.
        source_basename: String,
    }

    let mut records: Vec<Rec> = Vec::with_capacity(file_ids.len());
    for id in file_ids {
        match conn.query_row(
            "SELECT target_path, COALESCE(derived_date,''), COALESCE(derived_slug,''), \
                    COALESCE(caption_slug,''), COALESCE(ext,''), \
                    COALESCE(os_date,''), COALESCE(source_path,'') \
             FROM media WHERE id = ?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        ) {
            Ok((tp, dd, ds, cs, ex, od, sp)) => {
                let source_basename = Path::new(&sp)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                records.push(Rec {
                    id: id.clone(),
                    target_path: tp,
                    derived_date: dd,
                    derived_slug: ds,
                    caption_slug: cs,
                    ext: ex,
                    os_date: od,
                    source_basename,
                });
            }
            Err(e) => eprintln!("remove-slug: failed to load {id}: {e}"),
        }
    }

    // Sort for chronological counter assignment within each day:
    //   1. derived_date (day grouping)
    //   2. os_date — OS mtime recorded at import; includes time-of-day, so sorts
    //      chronologically when present (format `YYYY-MM-DD HH:MM:SS` sorts lexicographically).
    //   3. source_basename — original pre-import filename; camera sequences like
    //      `IMG_20251130_143022.jpg` or `DSC_0001.jpg` sort chronologically.
    //   4. target_path — final tie-breaker for stability.
    records.sort_by(|a, b| {
        a.derived_date
            .cmp(&b.derived_date)
            .then(a.os_date.cmp(&b.os_date))
            .then(a.source_basename.cmp(&b.source_basename))
            .then(a.target_path.cmp(&b.target_path))
    });

    // ── 2. Build exclusion sets ────────────────────────────────────────────
    let batch_ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
    let batch_basenames: HashSet<String> = records
        .iter()
        .filter_map(|r| {
            Path::new(&r.target_path)
                .file_name()?
                .to_str()
                .map(|s| s.to_string())
        })
        .collect();
    let batch_old_paths: HashSet<&str> = records.iter().map(|r| r.target_path.as_str()).collect();

    // ── 3. Compute base counter per day prefix (excluding entire batch) ────
    let mut day_next: HashMap<String, u32> = HashMap::new();
    for rec in &records {
        if rec.derived_date.len() < 10 {
            continue;
        }
        let year = &rec.derived_date[..4];
        let month = &rec.derived_date[5..7];
        let day = &rec.derived_date[8..10];
        let day_prefix = format!("{year}-{month}-{day}");
        if day_next.contains_key(&day_prefix) {
            continue;
        }

        // DB max excluding all batch IDs.
        let db_pattern = format!("{year}/{day_prefix}-%");
        let placeholders = (0..batch_ids.len())
            .map(|i| format!("?{}", i + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let db_query = format!(
            "SELECT COALESCE(MAX(counter), 0) FROM media \
             WHERE target_path LIKE ?1 AND status IN ('moved','trashed','deleted') \
             AND id NOT IN ({placeholders})"
        );
        let db_max: u32 = {
            let mut stmt = conn.prepare(&db_query)?;
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                vec![Box::new(db_pattern.clone())];
            for id in &batch_ids {
                params.push(Box::new(id.to_string()));
            }
            stmt.query_row(
                params
                    .iter()
                    .map(|p| p.as_ref())
                    .collect::<Vec<_>>()
                    .as_slice(),
                |row| row.get::<_, u32>(0),
            )
            .unwrap_or(0)
        };

        // FS max excluding all batch basenames.
        let mut fs_max: u32 = 0;
        let yr_dir = Path::new(target_root).join(year);
        if let Ok(rd) = std::fs::read_dir(&yr_dir) {
            for entry in rd.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if batch_basenames.contains(name) {
                        continue;
                    }
                    if name.starts_with(&format!("{day_prefix}-")) {
                        let rest = &name[day_prefix.len() + 1..];
                        let digits: String = rest.chars().take(4).collect();
                        if digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit()) {
                            if let Ok(n) = digits.parse::<u32>() {
                                if n > fs_max {
                                    fs_max = n;
                                }
                            }
                        }
                    }
                }
            }
        }

        day_next.insert(day_prefix, db_max.max(fs_max) + 1);
    }

    // ── 4. Pre-assign all new paths ────────────────────────────────────────
    // (no FS writes yet; `seen_new_paths` prevents intra-batch collisions)
    struct Assignment {
        new_target_path: String,
        stored_counter: Option<u32>,
    }
    let mut assignments: Vec<Assignment> = Vec::with_capacity(records.len());
    let mut seen_new_paths: HashSet<String> = HashSet::new();

    for rec in &records {
        if rec.derived_date.len() < 10 {
            assignments.push(Assignment {
                new_target_path: rec.target_path.clone(),
                stored_counter: None,
            });
            continue;
        }
        let year = &rec.derived_date[..4];
        let month = &rec.derived_date[5..7];
        let day = &rec.derived_date[8..10];
        let day_prefix = format!("{year}-{month}-{day}");

        let assignment = if !rec.caption_slug.is_empty() {
            let plain = format!("{year}/{day_prefix}-{}.{}", rec.caption_slug, rec.ext);
            // Collision: claimed by another batch file, or occupied by a non-batch path.
            let in_batch = seen_new_paths.contains(&plain);
            let on_disk = if !in_batch {
                let abs = Path::new(target_root).join(&plain);
                abs.exists() && !batch_old_paths.contains(plain.as_str())
            } else {
                false
            };
            let in_db = if !in_batch && !on_disk {
                let ph = (0..batch_ids.len())
                    .map(|i| format!("?{}", i + 2))
                    .collect::<Vec<_>>()
                    .join(", ");
                let q = format!(
                    "SELECT COUNT(*) FROM media WHERE target_path = ?1 AND id NOT IN ({ph}) \
                     AND status IN ('moved','trashed','deleted')"
                );
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                    vec![Box::new(plain.clone())];
                for id in &batch_ids {
                    params.push(Box::new(id.to_string()));
                }
                conn.prepare(&q)
                    .ok()
                    .and_then(|mut s| {
                        s.query_row(
                            params
                                .iter()
                                .map(|p| p.as_ref())
                                .collect::<Vec<_>>()
                                .as_slice(),
                            |row| row.get::<_, i64>(0),
                        )
                        .ok()
                    })
                    .unwrap_or(0)
                    > 0
            } else {
                false
            };

            if in_batch || on_disk || in_db {
                let counter = *day_next.get(&day_prefix).unwrap_or(&1);
                *day_next.get_mut(&day_prefix).unwrap() += 1;
                let p = format!(
                    "{year}/{day_prefix}-{}-{counter:04}.{}",
                    rec.caption_slug, rec.ext
                );
                seen_new_paths.insert(p.clone());
                Assignment { new_target_path: p, stored_counter: Some(counter) }
            } else {
                seen_new_paths.insert(plain.clone());
                Assignment { new_target_path: plain, stored_counter: None }
            }
        } else {
            let counter = *day_next.get(&day_prefix).unwrap_or(&1);
            *day_next.get_mut(&day_prefix).unwrap() += 1;
            let p = format!("{year}/{day_prefix}-{counter:04}.{}", rec.ext);
            seen_new_paths.insert(p.clone());
            Assignment { new_target_path: p, stored_counter: Some(counter) }
        };

        assignments.push(assignment);
    }

    // ── 5. Execute renames + DB updates ───────────────────────────────────
    let mut stats = RemoveSlugBatchStats { fixed: 0, skipped: 0, errors: Vec::new() };

    for (i, (rec, asgn)) in records.iter().zip(assignments.iter()).enumerate() {
        on_progress(i, &rec.id);

        if rec.derived_date.len() < 10 {
            stats.errors.push((
                rec.id.clone(),
                format!("derived_date '{}' is too short to parse", rec.derived_date),
            ));
            continue;
        }

        let old_abs = Path::new(target_root).join(&rec.target_path);
        let new_abs = Path::new(target_root).join(&asgn.new_target_path);

        if old_abs == new_abs && rec.derived_slug.is_empty() {
            stats.skipped += 1;
            continue;
        }

        if old_abs != new_abs && new_abs.exists() {
            stats.errors.push((
                rec.id.clone(),
                format!("remove-slug: target already exists: {}", new_abs.display()),
            ));
            continue;
        }

        if old_abs != new_abs && old_abs.exists() {
            if let Some(parent) = new_abs.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    stats.errors.push((rec.id.clone(), e.to_string()));
                    continue;
                }
            }
            if let Err(e) = std::fs::rename(&old_abs, &new_abs) {
                stats.errors.push((rec.id.clone(), e.to_string()));
                continue;
            }
        }

        // Save old slug as a tag of type "slug".
        if !rec.derived_slug.is_empty() {
            let existing_tag: Option<i64> = conn
                .query_row(
                    "SELECT id FROM tags WHERE name = ?1 AND type = 'slug'",
                    rusqlite::params![rec.derived_slug],
                    |row| row.get(0),
                )
                .optional()
                .unwrap_or(None);
            let tag_id: i64 = match existing_tag {
                Some(id) => id,
                None => {
                    conn.execute(
                        "INSERT INTO tags (name, type) VALUES (?1, 'slug')",
                        rusqlite::params![rec.derived_slug],
                    )?;
                    conn.last_insert_rowid()
                }
            };
            let _ = conn.execute(
                "INSERT OR IGNORE INTO media_tags (media_id, tag_id) VALUES (?1, ?2)",
                rusqlite::params![rec.id, tag_id],
            );
        }

        if let Err(e) = conn.execute(
            "UPDATE media SET derived_slug = NULL, target_path = ?1, counter = ?2 WHERE id = ?3",
            rusqlite::params![asgn.new_target_path, asgn.stored_counter, rec.id],
        ) {
            stats.errors.push((rec.id.clone(), e.to_string()));
            continue;
        }

        stats.fixed += 1;
    }

    Ok(stats)
}

/// Fix the OS mtime of a single imported media file on disk.
///
/// Re-derives the best timestamp using the same priority logic as the import
/// execute phase ([`crate::import::best_mtime`]):
/// 1. Full timestamp encoded in the original source filename.
/// 2. `derived_date` at noon UTC (source mtime unavailable for repair).
///
/// Returns `Ok(true)` when the mtime was updated, `Ok(false)` when the file
/// was skipped (no `derived_date` stored, or the target file does not exist on
/// disk — both are non-fatal).
pub fn fix_os_time(db_path: &str, target_root: &str, file_id: &str) -> Result<bool> {
    use std::path::Path;

    let conn = Connection::open(db_path)?;

    let (target_path, derived_date, source_path): (String, Option<String>, String) = conn
        .query_row(
            "SELECT COALESCE(target_path,''), derived_date, COALESCE(source_path,'') \
             FROM media WHERE id = ?1",
            [file_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

    if derived_date.is_none() {
        return Ok(false);
    }

    let filename_secs = {
        let base = Path::new(&source_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        crate::import::find_filename_timestamp(base)
    };

    let ft = match crate::import::best_mtime(filename_secs, None, derived_date.as_deref()) {
        Some(ft) => ft,
        None => return Ok(false),
    };

    let abs_tgt = Path::new(target_root).join(&target_path);
    if !abs_tgt.exists() {
        return Ok(false);
    }

    filetime::set_file_mtime(&abs_tgt, ft)?;
    Ok(true)
}

/// Mark media files as trashed (`status = 'trashed'`).
///
/// Returns the number of rows updated.
pub fn trash_files(db_path: &str, media_ids: &[String]) -> Result<usize> {
    if media_ids.is_empty() {
        return Ok(0);
    }
    let conn = Connection::open(db_path)?;
    let ph: String = std::iter::repeat("?").take(media_ids.len()).collect::<Vec<_>>().join(",");
    let updated = conn.execute(
        &format!("UPDATE media SET status = 'trashed' WHERE id IN ({ph})"),
        rusqlite::params_from_iter(media_ids),
    )?;
    Ok(updated)
}

/// Restore trashed media files to normal (`status = 'moved'`).
///
/// Returns the number of rows updated.
pub fn keep_files(db_path: &str, media_ids: &[String]) -> Result<usize> {
    if media_ids.is_empty() {
        return Ok(0);
    }
    let conn = Connection::open(db_path)?;
    let ph: String = std::iter::repeat("?").take(media_ids.len()).collect::<Vec<_>>().join(",");
    let updated = conn.execute(
        &format!("UPDATE media SET status = 'moved' WHERE id IN ({ph})"),
        rusqlite::params_from_iter(media_ids),
    )?;
    Ok(updated)
}

/// Load up to `limit` trashed files (`status = 'trashed'`), ordered by `target_path`.
pub fn load_trashed_files(db_path: &str, limit: usize) -> Result<Vec<MediaFile>> {
    let conn = Connection::open(db_path)?;

    let sql = format!(
        "SELECT m.id, m.target_path, COALESCE(m.derived_date,''), m.ext,
                COALESCE(GROUP_CONCAT(t.name || CHAR(30) || t.type, CHAR(31)),''),
                COALESCE(m.derived_slug,''), COALESCE(m.caption_slug,''),
                COALESCE(m.os_date,''), COALESCE(m.source_path,''),
                COALESCE(m.status,'trashed'),
                COALESCE(m.missing_on_disk,0)
         FROM media m
         LEFT JOIN media_tags mt ON mt.media_id = m.id
         LEFT JOIN tags t ON t.id = mt.tag_id
         WHERE m.status = 'trashed'
         GROUP BY m.id
         ORDER BY m.target_path
         LIMIT {limit}"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let tags_str: String = row.get(4)?;
        let (tags, tag_types) = if tags_str.is_empty() {
            (vec![], vec![])
        } else {
            tags_str.split('\x1f')
                .map(|pair| {
                    if let Some(sep) = pair.find('\x1e') {
                        (pair[..sep].to_string(), pair[sep + 1..].to_string())
                    } else {
                        (pair.to_string(), String::new())
                    }
                })
                .unzip()
        };
        let source_path: String = row.get(8)?;
        let orig_filename = std::path::Path::new(&source_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(MediaFile {
            id: row.get(0)?,
            target_path: row.get(1)?,
            derived_date: row.get(2)?,
            ext: row.get(3)?,
            tags,
            tag_types,
            derived_slug: row.get(5)?,
            caption_slug: row.get(6)?,
            os_date: row.get(7)?,
            orig_filename,
            status: row.get(9)?,
            missing_on_disk: row.get::<_, i64>(10)? != 0,
        })
    })?;

    let mut files = Vec::new();
    for r in rows {
        files.push(r?);
    }
    Ok(files)
}

/// Delete the given trashed files from the filesystem and mark them `status = 'deleted'` in DB.
///
/// Each file is processed independently; FS errors are non-fatal and counted.
/// Returns `(deleted, errors)`.
pub fn delete_trashed_from_fs(
    db_path: &str,
    target_root: &str,
    media_ids: &[String],
) -> Result<(usize, usize)> {
    if media_ids.is_empty() {
        return Ok((0, 0));
    }

    let conn = Connection::open(db_path)?;
    let ph: String = std::iter::repeat("?").take(media_ids.len()).collect::<Vec<_>>().join(",");

    let target_paths: Vec<(String, String)> = {
        let sql = format!("SELECT id, target_path FROM media WHERE id IN ({ph})");
        let mut stmt = conn.prepare(&sql)?;
        let result: Vec<(String, String)> = stmt.query_map(rusqlite::params_from_iter(media_ids), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
        result
    };

    let mut deleted = 0usize;
    let mut errors = 0usize;

    for (id, target_path) in &target_paths {
        let abs = std::path::Path::new(target_root).join(target_path);
        if abs.exists() {
            match std::fs::remove_file(&abs) {
                Ok(()) => {
                    conn.execute(
                        "UPDATE media SET status = 'deleted' WHERE id = ?1",
                        rusqlite::params![id],
                    )?;
                    deleted += 1;
                }
                Err(_) => {
                    errors += 1;
                }
            }
        } else {
            // File already gone from disk — still mark as deleted in DB.
            conn.execute(
                "UPDATE media SET status = 'deleted' WHERE id = ?1",
                rusqlite::params![id],
            )?;
            deleted += 1;
        }
    }

    Ok((deleted, errors))
}

/// Return the import source directories used in previous import sessions,
/// ordered by recency (most recent first), deduplicated.
///
/// Each `import-*` tag (type `mex`) covers exactly one `:import <path>` run.
/// The common path prefix of all `source_path` values for a tag group equals
/// the directory that was passed to `:import`.
pub fn load_recent_import_source_dirs(db_path: &str) -> Result<Vec<String>> {
    let conn = Connection::open(db_path)?;

    // Collect (tag_name, max_moved_at, source_path) rows for all import sessions.
    let mut stmt = conn.prepare(
        "SELECT t.name, MAX(m.moved_at), m.source_path \
         FROM tags t \
         JOIN media_tags mt ON mt.tag_id = t.id \
         JOIN media m ON m.id = mt.media_id \
         WHERE t.type = 'mex' AND t.name LIKE 'import-%' AND m.source_path IS NOT NULL \
         GROUP BY t.name, m.source_path \
         ORDER BY MAX(m.moved_at) DESC",
    )?;

    // Group source_paths by tag_name; maintain insertion order (rows come ordered by recency).
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let tag_name: String = row.get(0)?;
        let source_path: String = row.get(2)?;
        if let Some(entry) = groups.iter_mut().find(|(t, _)| t == &tag_name) {
            entry.1.push(source_path);
        } else {
            groups.push((tag_name, vec![source_path]));
        }
    }

    // For each group, compute the longest common path prefix.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut result: Vec<String> = Vec::new();
    for (_tag, paths) in groups {
        if paths.is_empty() {
            continue;
        }
        let root = common_path_prefix(&paths);
        if !root.is_empty() && seen.insert(root.clone()) {
            result.push(root);
        }
    }
    Ok(result)
}

/// Compute the longest common filesystem-path prefix of a list of absolute paths.
/// Returns the deepest directory that is an ancestor of all paths.
fn common_path_prefix(paths: &[String]) -> String {
    use std::path::Path;
    if paths.is_empty() {
        return String::new();
    }
    // Split each path into components and find common prefix component-by-component.
    let first_comps: Vec<_> = Path::new(&paths[0]).components().collect();
    let mut common_len = first_comps.len();
    for path in paths.iter().skip(1) {
        let comps: Vec<_> = Path::new(path).components().collect();
        common_len = common_len.min(comps.len());
        for (i, (a, b)) in first_comps.iter().zip(comps.iter()).enumerate() {
            if a != b {
                common_len = i;
                break;
            }
        }
    }
    if common_len == 0 {
        return String::new();
    }
    // Build the common prefix path from components.
    let mut prefix = std::path::PathBuf::new();
    for comp in first_comps.iter().take(common_len) {
        prefix.push(comp);
    }
    prefix.to_string_lossy().into_owned()
}

/// Count files with `status = 'trashed'`.
pub fn count_trashed(db_path: &str) -> Result<usize> {
    let conn = Connection::open(db_path)?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM media WHERE status = 'trashed'",
        [],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_file_date_day_format() {
        assert_eq!(
            rename_file_date("2022-04-18-0001.jpg", "2023-06-15"),
            "2023-06-15-0001.jpg"
        );
    }

    #[test]
    fn rename_file_date_day_format_no_rest() {
        assert_eq!(
            rename_file_date("2022-04-18.jpg", "2023-06-15"),
            "2023-06-15.jpg"
        );
    }

    #[test]
    fn rename_file_date_slug_format() {
        assert_eq!(
            rename_file_date("2022-04-festival-0001.jpg", "2023-06-15"),
            "2023-06-festival-0001.jpg"
        );
    }

    #[test]
    fn rename_file_date_slug_format_caption() {
        assert_eq!(
            rename_file_date("2022-04-summer-0001-beach.jpeg", "2020-12-01"),
            "2020-12-summer-0001-beach.jpeg"
        );
    }

    #[test]
    fn rename_file_date_unrecognised_is_unchanged() {
        let name = "not-a-date-filename.jpg";
        assert_eq!(rename_file_date(name, "2023-06-15"), name);
    }

    #[test]
    fn days_from_epoch_unix_origin() {
        assert_eq!(days_from_epoch(1970, 1, 1), 0);
    }

    #[test]
    fn days_from_epoch_known_date() {
        // 2024-01-01 → days since 1970-01-01
        // Python: (date(2024,1,1)-date(1970,1,1)).days = 19723
        assert_eq!(days_from_epoch(2024, 1, 1), 19723);
    }

    #[test]
    fn extract_hms_from_str_valid() {
        assert_eq!(
            extract_hms_from_str("2022-04-18 14:30:55"),
            Some((14, 30, 55))
        );
    }

    #[test]
    fn extract_hms_from_str_midnight() {
        assert_eq!(
            extract_hms_from_str("2022-04-18 00:00:00"),
            Some((0, 0, 0))
        );
    }

    #[test]
    fn extract_hms_from_str_too_short() {
        assert_eq!(extract_hms_from_str("2022-04-18"), None);
        assert_eq!(extract_hms_from_str(""), None);
    }

    #[test]
    fn datetime_to_filetime_midnight() {
        // 1970-01-01 00:00:00 → Unix epoch 0
        let ft = datetime_to_filetime("1970-01-01", 0, 0, 0);
        assert_eq!(ft.unix_seconds(), 0);
    }

    #[test]
    fn fix_date_succeeds_and_updates_db() {
        use rusqlite::Connection;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("mex_test_{}", std::process::id()));
        let old_year_dir = dir.join("2022");
        let new_year_dir = dir.join("2023");
        fs::create_dir_all(&old_year_dir).unwrap();
        fs::create_dir_all(&new_year_dir).unwrap();

        let old_name = "2022-04-18-0001.jpg";
        let new_name = "2023-06-15-0001.jpg";
        let old_file = old_year_dir.join(old_name);
        fs::write(&old_file, b"data").unwrap();

        // Set up a real SQLite DB.
        let db_path = dir.join("mex.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (id TEXT PRIMARY KEY, target_path TEXT, derived_date TEXT, ext TEXT, os_date TEXT, derived_slug TEXT, caption_slug TEXT);
             CREATE TABLE media_tags (media_id TEXT, tag_id TEXT);
             CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT);
             INSERT INTO media VALUES ('id1', '2022/2022-04-18-0001.jpg', '2022-04-18', 'jpg', '2022-04-18 10:00:00', '', '');",
        ).unwrap();
        drop(conn);

        let result = fix_date(
            db_path.to_str().unwrap(),
            dir.to_str().unwrap(),
            "id1",
            "2023-06-15",
        );
        assert!(result.is_ok(), "fix_date should succeed: {:?}", result);

        // Old file gone, new file present.
        assert!(!old_file.exists(), "old file should be gone after rename");
        assert!(new_year_dir.join(new_name).exists(), "new file should exist");

        // DB should reflect new path and date.
        let conn = Connection::open(&db_path).unwrap();
        let (tp, dd, od): (String, String, String) = conn
            .query_row("SELECT target_path, derived_date, os_date FROM media WHERE id='id1'", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(tp, "2023/2023-06-15-0001.jpg");
        assert_eq!(dd, "2023-06-15");
        assert!(od.starts_with("2023-06-15"), "os_date should start with new date, got {od}");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fix_date_absent_file_updates_db_only() {
        use rusqlite::Connection;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("mex_test_absent_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        let db_path = dir.join("mex.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (id TEXT PRIMARY KEY, target_path TEXT, derived_date TEXT, ext TEXT, os_date TEXT, derived_slug TEXT, caption_slug TEXT);
             CREATE TABLE media_tags (media_id TEXT, tag_id TEXT);
             CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT);
             INSERT INTO media VALUES ('id2', '2022/2022-04-18-0001.jpg', '2022-04-18', 'jpg', NULL, '', '');",
        ).unwrap();
        drop(conn);

        // File does not exist on disk — fix_date should still update the DB.
        let result = fix_date(
            db_path.to_str().unwrap(),
            dir.to_str().unwrap(),
            "id2",
            "2023-06-15",
        );
        assert!(result.is_ok(), "fix_date on absent file should succeed: {:?}", result);

        let conn = Connection::open(&db_path).unwrap();
        let (tp, dd): (String, String) = conn
            .query_row("SELECT target_path, derived_date FROM media WHERE id='id2'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(tp, "2023/2023-06-15-0001.jpg");
        assert_eq!(dd, "2023-06-15");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_folder_of() {
        assert_eq!(folder_of("2022/image.jpg"), "2022");
        assert_eq!(folder_of("a/b/c.jpg"), "a/b");
        assert_eq!(folder_of("image.jpg"), ".");
        assert_eq!(folder_of(""), ".");
        assert_eq!(folder_of("/image.jpg"), "");
        assert_eq!(folder_of("folder/"), "folder");
        assert_eq!(folder_of("a///b"), "a//");
        assert_eq!(folder_of("/"), "");
    }
}
