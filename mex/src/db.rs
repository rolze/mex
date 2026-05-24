use anyhow::Result;
use regex::Regex;
use rusqlite::{Connection, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::OnceLock,
};

pub const PATH_RE: &str = r"^(?P<year>\d{4})/(?:\d{4})-(?P<month>0[1-9]|1[0-2])-(?:(?P<day>0[1-9]|[12]\d|3[01])-(?:(?P<day_cap>[a-z0-9-]+)(?:\-(?P<day_coll>\d+))?|(?P<day_cnt>\d{4}))|(?P<slug>[a-z0-9-]{3,})-(?P<slug_cnt>\d{4})(?:\-(?P<slug_cap>[a-z0-9-]+))?)\.(?P<ext>[a-z0-9]+)$";

static PATH_REGEX: OnceLock<Regex> = OnceLock::new();

/// Returns the compiled [`PATH_RE`] regex, compiling it exactly once.
pub fn path_re() -> &'static Regex {
    PATH_REGEX.get_or_init(|| Regex::new(PATH_RE).unwrap())
}

/// Extract the slug from a `target_path` using the compiled PATH_RE regex.
/// Returns an empty string if no slug capture is present (day-based files).
pub fn slug_from_path(path: &str) -> String {
    path_re()
        .captures(path)
        .and_then(|c| c.name("slug").map(|m| m.as_str().to_string()))
        .unwrap_or_default()
}

// ── Sort helpers ─────────────────────────────────────────────────────────────

/// Returns a sort key `(base, collision)` for a `target_path` so that
/// collision files (`cap-2.ext`, `cap-3.ext`, …) sort immediately after
/// their origin (`cap.ext`) rather than before it.
///
/// A trailing `-N` before the final extension is treated as a collision
/// counter.  All other paths (no digit suffix) get collision = 0.
///
/// Examples:
/// * `"2024/2024-01-01-chisel.mp4"`   → `("2024/2024-01-01-chisel", 0)`
/// * `"2024/2024-01-01-chisel-2.mp4"` → `("2024/2024-01-01-chisel", 2)`
/// * `"2024/2024-01-01-chisel-wood.mp4"` → `("2024/2024-01-01-chisel-wood", 0)`
fn path_sort_key(path: &str) -> (&str, u64) {
    static COLL_RE: OnceLock<Regex> = OnceLock::new();
    let re = COLL_RE.get_or_init(|| Regex::new(r"^(.*)-(\d+)\.[^./]+$").unwrap());

    if let Some(caps) = re.captures(path) {
        let base_end = caps.get(1).unwrap().end();
        let n: u64 = caps[2].parse().unwrap_or(0);
        return (&path[..base_end], n);
    }

    // Strip extension to get a comparable base (so "cap.ext" key == "cap" not "cap.ext").
    static EXT_RE: OnceLock<Regex> = OnceLock::new();
    let ext_re = EXT_RE.get_or_init(|| Regex::new(r"^(.*)\.[^./]+$").unwrap());
    if let Some(caps) = ext_re.captures(path) {
        let base_end = caps.get(1).unwrap().end();
        return (&path[..base_end], 0);
    }

    (path, 0)
}

// ── Shared naming helpers ────────────────────────────────────────────────────

/// Scan `yr_dir` for the highest numeric counter under the filename pattern
/// `{prefix}-{N}.*`, skipping any names listed in `skip_basenames`.
///
/// The counter `N` is the digit run immediately after the final `-{prefix}-`
/// separator and before the next `.` or end of filename, so both zero-padded
/// (`0001`) and plain (`2`, `3`, …) forms are handled.  Returns 0 if no
/// matching file is found.
pub(crate) fn fs_counter_max(yr_dir: &Path, prefix: &str, skip_basenames: &HashSet<String>) -> u32 {
    let search_prefix = format!("{prefix}-");
    let mut max: u32 = 0;
    if let Ok(rd) = std::fs::read_dir(yr_dir) {
        for entry in rd.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if skip_basenames.contains(name) {
                    continue;
                }
                if let Some(rest) = name.strip_prefix(&search_prefix) {
                    // digits up to the first non-digit character
                    let digit_end = rest
                        .find(|c: char| !c.is_ascii_digit())
                        .unwrap_or(rest.len());
                    if digit_end > 0 {
                        if let Ok(n) = rest[..digit_end].parse::<u32>() {
                            if n > max {
                                max = n;
                            }
                        }
                    }
                }
            }
        }
    }
    max
}

/// Rename `old_rel` → `new_rel` relative to `target_root`.
/// Creates the parent directory if needed.
/// No-ops silently when `old_rel == new_rel` or the source file is absent on disk.
pub(crate) fn rename_file_rel(
    target_root: &Path,
    old_rel: &str,
    new_rel: &str,
) -> anyhow::Result<()> {
    let old_abs = target_root.join(old_rel);
    let new_abs = target_root.join(new_rel);
    if old_abs == new_abs {
        return Ok(());
    }
    if old_abs.exists() {
        if let Some(parent) = new_abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&old_abs, &new_abs)?;
    }
    Ok(())
}

/// Determine the target path for a caption-only file (no event slug).
///
/// Tries the plain form `{year}/{date_prefix}-{caption}.{ext}` first.
/// If that path is occupied — present in `seen_paths`, existing on disk and not
/// in `skip_disk_paths`, or in the DB and not among `exclude_ids` — falls back
/// to `{year}/{date_prefix}-{caption}-{N}.{ext}` with a counter starting at **2**
/// (the plain path is implicitly "version 1").
///
/// Per-caption counter state is kept in `caption_counters` across calls so that
/// consecutive collisions produce consecutive values.
///
/// Returns `(target_path, counter)` where `counter` is `None` for the plain form.
#[allow(clippy::too_many_arguments)]
pub(crate) fn next_caption_path(
    conn: &Connection,
    target_root: &Path,
    year: &str,
    date_prefix: &str,
    caption: &str,
    ext: &str,
    seen_paths: &HashSet<String>,
    skip_disk_paths: &HashSet<&str>,
    exclude_ids: &[&str],
    caption_counters: &mut HashMap<String, u32>,
) -> (String, Option<u32>) {
    let plain = format!("{year}/{date_prefix}-{caption}.{ext}");

    let in_seen = seen_paths.contains(&plain);
    let on_disk = !in_seen && {
        let abs = target_root.join(&plain);
        abs.exists() && !skip_disk_paths.contains(plain.as_str())
    };
    let in_db = !in_seen && !on_disk && {
        if exclude_ids.is_empty() {
            conn.query_row(
                "SELECT 1 FROM media WHERE target_path = ?1 \
                 AND status IN ('moved','trashed','deleted')",
                rusqlite::params![plain],
                |_| Ok(()),
            )
            .is_ok()
        } else {
            let ph = (0..exclude_ids.len())
                .map(|i| format!("?{}", i + 2))
                .collect::<Vec<_>>()
                .join(", ");
            let q = format!(
                "SELECT 1 FROM media WHERE target_path = ?1 \
                 AND id NOT IN ({ph}) AND status IN ('moved','trashed','deleted')"
            );
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(plain.clone())];
            for id in exclude_ids {
                params.push(Box::new(id.to_string()));
            }
            conn.prepare(&q)
                .ok()
                .and_then(|mut stmt| {
                    stmt.query_row(
                        params
                            .iter()
                            .map(|p| p.as_ref())
                            .collect::<Vec<_>>()
                            .as_slice(),
                        |_| Ok(()),
                    )
                    .ok()
                })
                .is_some()
        }
    };

    if !in_seen && !on_disk && !in_db {
        return (plain, None);
    }

    // Collision: find or initialise per-caption counter (minimum 2).
    let cap_key = format!("{date_prefix}-{caption}");
    if !caption_counters.contains_key(&cap_key) {
        let cap_pattern = format!("{year}/{date_prefix}-{caption}-%");
        let db_max: u32 = if exclude_ids.is_empty() {
            conn.query_row(
                "SELECT COALESCE(MAX(counter), 0) FROM media \
                 WHERE target_path LIKE ?1 AND status IN ('moved','trashed','deleted')",
                rusqlite::params![cap_pattern],
                |row| row.get::<_, u32>(0),
            )
            .unwrap_or(0)
        } else {
            let ph = (0..exclude_ids.len())
                .map(|i| format!("?{}", i + 2))
                .collect::<Vec<_>>()
                .join(", ");
            let q = format!(
                "SELECT COALESCE(MAX(counter), 0) FROM media \
                 WHERE target_path LIKE ?1 AND status IN ('moved','trashed','deleted') \
                 AND id NOT IN ({ph})"
            );
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(cap_pattern)];
            for id in exclude_ids {
                params.push(Box::new(id.to_string()));
            }
            conn.prepare(&q)
                .ok()
                .and_then(|mut stmt| {
                    stmt.query_row(
                        params
                            .iter()
                            .map(|p| p.as_ref())
                            .collect::<Vec<_>>()
                            .as_slice(),
                        |row| row.get::<_, u32>(0),
                    )
                    .ok()
                })
                .unwrap_or(0)
        };
        // Derive skip-basenames from the full relative paths in skip_disk_paths.
        let skip_basenames: HashSet<String> = skip_disk_paths
            .iter()
            .filter_map(|p| Path::new(p).file_name()?.to_str().map(|s| s.to_string()))
            .collect();
        let yr_dir = target_root.join(year);
        let fs_max = fs_counter_max(
            &yr_dir,
            &format!("{date_prefix}-{caption}"),
            &skip_basenames,
        );
        // Start at 2 (plain is "version 1"); respect any existing higher values.
        caption_counters.insert(cap_key.clone(), db_max.max(fs_max).max(1) + 1);
    }

    let ctr = *caption_counters.get(&cap_key).unwrap();
    *caption_counters.get_mut(&cap_key).unwrap() += 1;
    (
        format!("{year}/{date_prefix}-{caption}-{ctr}.{ext}"),
        Some(ctr),
    )
}

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

pub fn load_files(conn: &Connection) -> Result<Vec<MediaFile>> {
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
                 AND m.status != 'deleted'
               GROUP BY m.id
               ORDER BY m.target_path";

    let mut stmt = conn.prepare(sql)?;

    let rows = stmt.query_map([], |row| {
        let tags_str: String = row.get(4)?;
        let (tags, tag_types) = if tags_str.is_empty() {
            (vec![], vec![])
        } else {
            tags_str
                .split('\x1f')
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
    files.sort_by(|a, b| {
        let ka = path_sort_key(&a.target_path);
        let kb = path_sort_key(&b.target_path);
        ka.0.cmp(kb.0).then(ka.1.cmp(&kb.1))
    });
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
pub fn init_db(conn: &Connection) -> Result<()> {
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
    ensure_schema_v1(conn)?;
    ensure_schema_v2(conn)?;
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
pub fn set_missing_on_disk(conn: &Connection, media_id: &str, missing: bool) -> Result<()> {
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
    conn: &mut Connection,
    media_ids: &[String],
    tag_name: &str,
    tag_type: Option<&str>,
) -> Result<String> {
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
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO media_tags (media_id, tag_id) VALUES (?1, ?2)",
            )?;
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
pub fn remove_tags(conn: &Connection, media_ids: &[String], tag_names: &[String]) -> Result<usize> {
    if media_ids.is_empty() {
        return Ok(0);
    }

    let id_ph: String = std::iter::repeat_n("?", media_ids.len())
        .collect::<Vec<_>>()
        .join(",");

    let removed = if tag_names.is_empty() {
        conn.execute(
            &format!("DELETE FROM media_tags WHERE media_id IN ({id_ph})"),
            rusqlite::params_from_iter(media_ids),
        )?
    } else {
        let name_ph: String = std::iter::repeat_n("?", tag_names.len())
            .collect::<Vec<_>>()
            .join(",");
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
        && (basename.len() == 10
            || basename.as_bytes()[10] == b'.'
            || basename.as_bytes()[10] == b'-');

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
pub fn fix_date(conn: &Connection, target_root: &str, file_id: &str, new_date: &str) -> Result<()> {
    use filetime::{set_file_mtime, FileTime};
    use std::path::Path;

    // Fetch current target_path and stored os_date (as fallback for time component).
    let (target_path, old_os_date): (String, Option<String>) = conn.query_row(
        "SELECT target_path, os_date FROM media WHERE id = ?1",
        [file_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // Split into folder prefix and basename.
    let basename = if let Some(pos) = target_path.rfind('/') {
        &target_path[pos + 1..]
    } else {
        target_path.as_str()
    };

    let new_basename = rename_file_date(basename, new_date);
    let new_year = &new_date[..4];
    // Keep the same sub-folder structure (year folder).
    let new_folder = new_year.to_string();
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
            anyhow::anyhow!("mtime update not supported on this filesystem (exFAT/WSL2?): {e}")
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

/// Read the hh:mm:ss components from a file's mtime (UTC time-of-day).
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
    if s.len() < 19 {
        return None;
    }
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
pub fn fix_ext(conn: &Connection, target_root: &str, file_id: &str, new_ext: &str) -> Result<()> {
    use std::path::Path;

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

/// Fix (add / edit / remove) the caption slug of a single media file.
///
/// `new_caption` must already be a valid kebab-case caption (lowercase, `a-z0-9-`, max 42
/// chars) or an empty string (to remove the caption entirely).
///
/// Filename surgery is regex-driven:
/// - The `target_path` is parsed using the definitive regex from `REGEXP.md`.
/// - All structural components are preserved, and the new caption is substituted.
/// - If the base path is occupied, the max occupied counter is pre-computed from a single
///   `MAX(counter)` DB query and a single filesystem scan; the new counter is set to
///   `max + 1` and the path is generated directly (no unbounded loop).
pub fn fix_caption(
    conn: &Connection,
    target_root: &str,
    file_id: &str,
    new_caption: &str,
) -> Result<()> {
    use std::path::Path;

    let target_path: String = conn.query_row(
        "SELECT target_path FROM media WHERE id = ?1",
        [file_id],
        |row| row.get(0),
    )?;

    let caps = path_re()
        .captures(&target_path)
        .ok_or_else(|| anyhow::anyhow!("file path does not match formal spec: {target_path}"))?;

    let year = caps.name("year").map(|m| m.as_str()).unwrap_or("0000");
    let month = caps.name("month").map(|m| m.as_str()).unwrap_or("00");
    let ext = caps.name("ext").map(|m| m.as_str()).unwrap_or("bin");

    // Rebuild builder
    let build_with_collision = |cap: &str, coll: Option<&str>| -> String {
        let stem = if let Some(day_m) = caps.name("day") {
            let day = day_m.as_str();
            if cap.is_empty() {
                format!(
                    "{year}-{month}-{day}-{}",
                    caps.name("day_cnt").map(|m| m.as_str()).unwrap_or("0001")
                )
            } else if let Some(c) = coll {
                format!("{year}-{month}-{day}-{cap}-{c}")
            } else {
                format!("{year}-{month}-{day}-{cap}")
            }
        } else if let Some(slug_m) = caps.name("slug") {
            let slug = slug_m.as_str();
            let cnt = caps.name("slug_cnt").map(|m| m.as_str()).unwrap_or("0001");
            if cap.is_empty() {
                format!("{year}-{month}-{slug}-{cnt}")
            } else {
                format!("{year}-{month}-{slug}-{cnt}-{cap}")
            }
        } else {
            "unknown".to_string()
        };

        format!("{year}/{stem}.{ext}")
    };

    let base_new_path =
        build_with_collision(new_caption, caps.name("day_coll").map(|m| m.as_str()));

    let mut final_path = base_new_path;

    // Initial counter for the new path — determined before collision check.
    // Slug-type: preserve the existing slug counter unchanged.
    // Day-type + removing caption: result is a day-counter file.
    // Day-type + adding/changing caption: plain caption = NULL counter.
    let mut new_counter: Option<u32> = if caps.name("slug").is_some() {
        caps.name("slug_cnt").and_then(|m| m.as_str().parse().ok())
    } else if new_caption.is_empty() {
        caps.name("day_cnt")
            .map(|m| m.as_str())
            .unwrap_or("0001")
            .parse()
            .ok()
    } else {
        None
    };

    if final_path != target_path {
        let base_on_disk = Path::new(target_root).join(&final_path).exists();
        let base_in_db = !base_on_disk
            && conn
                .query_row(
                    "SELECT COUNT(*) FROM media \
                     WHERE target_path = ?1 AND id != ?2 \
                     AND status IN ('moved','trashed','deleted')",
                    rusqlite::params![final_path, file_id],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0;

        if base_on_disk || base_in_db {
            // Pre-compute max occupied counter once (one DB query + one FS scan),
            // then generate the collision path directly — no unbounded loop.
            let (db_pattern, fs_prefix) = if let Some(day_m) = caps.name("day") {
                let day = day_m.as_str();
                if new_caption.is_empty() {
                    (
                        format!("{year}/{year}-{month}-{day}-%"),
                        format!("{year}-{month}-{day}"),
                    )
                } else {
                    (
                        format!("{year}/{year}-{month}-{day}-{new_caption}-%"),
                        format!("{year}-{month}-{day}-{new_caption}"),
                    )
                }
            } else {
                let slug = caps.name("slug").map(|m| m.as_str()).unwrap_or("");
                (
                    format!("{year}/{year}-{month}-{slug}-%"),
                    format!("{year}-{month}-{slug}"),
                )
            };

            let db_max: u32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(counter), 0) FROM media \
                     WHERE target_path LIKE ?1 AND id != ?2 \
                     AND status IN ('moved','trashed','deleted')",
                    rusqlite::params![db_pattern, file_id],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap_or(0);

            let yr_dir = Path::new(target_root).join(year);
            let mut skip_basename = HashSet::new();
            if let Some(b) = Path::new(&target_path).file_name().and_then(|n| n.to_str()) {
                skip_basename.insert(b.to_string());
            }
            let fs_max = fs_counter_max(&yr_dir, &fs_prefix, &skip_basename);
            let counter = db_max.max(fs_max).max(1) + 1;

            final_path = if let Some(day_m) = caps.name("day") {
                let day = day_m.as_str();
                if new_caption.is_empty() {
                    format!("{year}/{year}-{month}-{day}-{counter:04}.{ext}")
                } else {
                    format!("{year}/{year}-{month}-{day}-{new_caption}-{counter}.{ext}")
                }
            } else {
                let slug = caps.name("slug").map(|m| m.as_str()).unwrap_or("");
                let cap_part = if new_caption.is_empty() {
                    String::new()
                } else {
                    format!("-{new_caption}")
                };
                format!("{year}/{year}-{month}-{slug}-{counter:04}{cap_part}.{ext}")
            };
            // Collision always stores the resolved counter.
            new_counter = Some(counter);
        }
    }

    // Rename on disk
    rename_file_rel(Path::new(target_root), &target_path, &final_path)?;

    // Update DB — counter must be kept accurate so that db_max queries by
    // other commands (e.g. deslugify_batch) see the correct value.
    // Plain caption files store NULL; all counter-bearing formats store the
    // numeric value.
    let stored_caption = if new_caption.is_empty() {
        None
    } else {
        Some(new_caption)
    };
    conn.execute(
        "UPDATE media SET target_path = ?1, caption_slug = ?2, counter = ?3 WHERE id = ?4",
        rusqlite::params![final_path, stored_caption, new_counter, file_id],
    )?;

    Ok(())
}

/// Per-file result produced by [`deslugify_batch`].
#[derive(Debug)]
pub struct DeslugifyBatchStats {
    pub fixed: usize,
    pub skipped: usize,
    /// Per-file errors: `(file_id, message)`.
    pub errors: Vec<(String, String)>,
}

/// Remove slugs from a batch of media files and assign fresh day-precision counters.
///
/// The slug for each file is derived from its `target_path` filename (REGEXP.md);
/// the legacy `derived_slug` DB column is not read or written.
///
/// All counter values are pre-computed **before** any file is renamed, so selecting
/// every file from a given day restarts the counter at 0001.
///
/// `on_progress(index, file_id)` is called before each file is processed.
pub fn deslugify_batch(
    conn: &Connection,
    target_root: &str,
    file_ids: &[String],
    on_progress: impl Fn(usize, &str),
) -> Result<DeslugifyBatchStats> {
    use std::path::Path;

    if file_ids.is_empty() {
        return Ok(DeslugifyBatchStats {
            fixed: 0,
            skipped: 0,
            errors: Vec::new(),
        });
    }

    // ── 1. Load all records ────────────────────────────────────────────────
    struct Rec {
        id: String,
        target_path: String,
        derived_date: String,
        /// Slug parsed from `target_path` at runtime (REGEXP.md); empty if no slug.
        filename_slug: String,
        caption_slug: String,
        ext: String,
        os_date: String,
        source_basename: String,
    }

    let mut records: Vec<Rec> = Vec::with_capacity(file_ids.len());
    for id in file_ids {
        match conn.query_row(
            "SELECT target_path, COALESCE(derived_date,''), \
                    COALESCE(caption_slug,''), COALESCE(ext,''), \
                    COALESCE(os_date,''), COALESCE(source_path,'') \
             FROM media WHERE id = ?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        ) {
            Ok((tp, dd, cs, ex, od, sp)) => {
                let source_basename = Path::new(&sp)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let filename_slug = slug_from_path(&tp);
                records.push(Rec {
                    id: id.clone(),
                    target_path: tp,
                    derived_date: dd,
                    filename_slug,
                    caption_slug: cs,
                    ext: ex,
                    os_date: od,
                    source_basename,
                });
            }
            Err(e) => eprintln!("deslugify: failed to load {id}: {e}"),
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
        // Exclude caption files (caption_slug IS NOT NULL) from the day
        // counter: their counter column does not represent a position in the
        // sequential day counter and must not inflate db_max.
        let db_query = format!(
            "SELECT COALESCE(MAX(counter), 0) FROM media \
             WHERE target_path LIKE ?1 AND status IN ('moved','trashed','deleted') \
             AND caption_slug IS NULL \
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
        let yr_dir = Path::new(target_root).join(year);
        let fs_max = fs_counter_max(&yr_dir, &day_prefix, &batch_basenames);

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
    let mut caption_counters: HashMap<String, u32> = HashMap::new();

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
            let (path, stored_counter) = next_caption_path(
                conn,
                Path::new(target_root),
                year,
                &day_prefix,
                &rec.caption_slug,
                &rec.ext,
                &seen_new_paths,
                &batch_old_paths,
                &batch_ids,
                &mut caption_counters,
            );
            seen_new_paths.insert(path.clone());
            Assignment {
                new_target_path: path,
                stored_counter,
            }
        } else {
            let counter = *day_next.get(&day_prefix).unwrap_or(&1);
            *day_next.get_mut(&day_prefix).unwrap() += 1;
            let p = format!("{year}/{day_prefix}-{counter:04}.{}", rec.ext);
            seen_new_paths.insert(p.clone());
            Assignment {
                new_target_path: p,
                stored_counter: Some(counter),
            }
        };

        assignments.push(assignment);
    }

    // ── 5. Execute renames + DB updates ───────────────────────────────────
    let mut stats = DeslugifyBatchStats {
        fixed: 0,
        skipped: 0,
        errors: Vec::new(),
    };

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

        if old_abs == new_abs && rec.filename_slug.is_empty() {
            stats.skipped += 1;
            continue;
        }

        if old_abs != new_abs && new_abs.exists() {
            stats.errors.push((
                rec.id.clone(),
                format!("deslugify: target already exists: {}", new_abs.display()),
            ));
            continue;
        }

        if let Err(e) = rename_file_rel(
            Path::new(target_root),
            &rec.target_path,
            &asgn.new_target_path,
        ) {
            stats.errors.push((rec.id.clone(), e.to_string()));
            continue;
        }

        // Save old slug as a tag of type "slug".
        if !rec.filename_slug.is_empty() {
            let existing_tag: Option<i64> = conn
                .query_row(
                    "SELECT id FROM tags WHERE name = ?1 AND type = 'slug'",
                    rusqlite::params![rec.filename_slug],
                    |row| row.get(0),
                )
                .optional()
                .unwrap_or(None);
            let tag_id: i64 = match existing_tag {
                Some(id) => id,
                None => {
                    conn.execute(
                        "INSERT INTO tags (name, type) VALUES (?1, 'slug')",
                        rusqlite::params![rec.filename_slug],
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
            "UPDATE media SET target_path = ?1, counter = ?2 WHERE id = ?3",
            rusqlite::params![asgn.new_target_path, asgn.stored_counter, rec.id],
        ) {
            stats.errors.push((rec.id.clone(), e.to_string()));
            continue;
        }

        stats.fixed += 1;
    }

    Ok(stats)
}

// ── Slugify ───────────────────────────────────────────────────────────────────

/// Which execution mode was chosen by [`slugify_batch`].
#[derive(Debug, PartialEq, Eq)]
pub enum SlugifyBatchMode {
    /// All files had the same slug X, which equals the new slug, and the
    /// selection covers every file with slug X in that `yyyy-mm` → paths were
    /// rewritten in-place (counters preserved).
    Rename,
    /// Files were merged / recounted under the new slug.
    Assign,
}

/// Per-batch result produced by [`slugify_batch`].
#[derive(Debug)]
pub struct SlugifyBatchStats {
    pub mode: SlugifyBatchMode,
    pub fixed: usize,
    pub skipped: usize,
    /// Per-file errors: `(file_id, message)`.
    pub errors: Vec<(String, String)>,
}

/// Group a batch of media files under `new_slug`.
///
/// **Guards** (checked before any work is done):
/// - All selected files must share the same `yyyy-mm` prefix → else error returned.
/// - The selection may contain files from at most ONE distinct existing slug → else error.
///
/// **Rename mode** — activated when the entire set of files with the *existing* slug in
/// `yyyy-mm` is selected AND no no-slug files are mixed in.  Only the slug token in
/// filenames is swapped; counters are preserved.  DB: only `target_path` updated.
///
/// **Assign mode** — everything else.  Counter-slot assignment is pre-computed before
/// any rename executes (safe for idempotent re-runs).
///
/// The `derived_slug` DB column is **not** read or written (legacy).
///
/// `on_progress(index, file_id)` is called before each file is processed.
pub fn slugify_batch(
    conn: &Connection,
    target_root: &str,
    file_ids: &[String],
    new_slug: &str,
    on_progress: impl Fn(usize, &str),
) -> Result<SlugifyBatchStats> {
    use std::path::Path;

    if file_ids.is_empty() {
        return Ok(SlugifyBatchStats {
            mode: SlugifyBatchMode::Assign,
            fixed: 0,
            skipped: 0,
            errors: Vec::new(),
        });
    }

    // ── Load batch records ───────────────────────────────────────────────────
    #[derive(Debug)]
    struct BatchRec {
        id: String,
        target_path: String,
        /// Year string, e.g. "2025".
        year: String,
        /// Month string, e.g. "11".
        month: String,
        /// Full day string, e.g. "30" (empty for slug-based files without a day).
        day: String,
        filename_slug: String,
        caption_slug: String,
        ext: String,
        os_date: String,
        source_basename: String,
    }

    let mut batch: Vec<BatchRec> = Vec::with_capacity(file_ids.len());
    for id in file_ids {
        let row = conn.query_row(
            "SELECT target_path, COALESCE(derived_date,''), \
                    COALESCE(caption_slug,''), COALESCE(ext,''), \
                    COALESCE(os_date,''), COALESCE(source_path,'') \
             FROM media WHERE id = ?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        );
        match row {
            Ok((tp, derived_date, cs, ex, od, sp)) => {
                let caps = path_re().captures(&tp);
                let year = caps
                    .as_ref()
                    .and_then(|c| c.name("year"))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| derived_date.get(..4).unwrap_or("").to_string());
                let month = caps
                    .as_ref()
                    .and_then(|c| c.name("month"))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| derived_date.get(5..7).unwrap_or("").to_string());
                let day = caps
                    .as_ref()
                    .and_then(|c| c.name("day"))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let filename_slug = caps
                    .as_ref()
                    .and_then(|c| c.name("slug"))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let source_basename = Path::new(&sp)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                batch.push(BatchRec {
                    id: id.clone(),
                    target_path: tp,
                    year,
                    month,
                    day,
                    filename_slug,
                    caption_slug: cs,
                    ext: ex,
                    os_date: od,
                    source_basename,
                });
            }
            Err(e) => {
                return Err(anyhow::anyhow!("slugify: failed to load {id}: {e}"));
            }
        }
    }

    // ── Multi-month guard ────────────────────────────────────────────────────
    let yr_month: HashSet<String> = batch
        .iter()
        .map(|r| format!("{}-{}", r.year, r.month))
        .collect();
    if yr_month.len() > 1 {
        return Err(anyhow::anyhow!(
            "slugify: all selected files must share the same yyyy-mm prefix"
        ));
    }
    let ym = yr_month.into_iter().next().unwrap();
    let (batch_year, batch_month) = ym.split_once('-').unwrap();
    let batch_year = batch_year.to_string();
    let batch_month = batch_month.to_string();

    // ── Multi-slug guard ─────────────────────────────────────────────────────
    let distinct_slugs: HashSet<&str> = batch
        .iter()
        .filter(|r| !r.filename_slug.is_empty())
        .map(|r| r.filename_slug.as_str())
        .collect();
    if distinct_slugs.len() > 1 {
        return Err(anyhow::anyhow!(
            "slugify: cannot mix files from different slug groups — select from one slug at a time"
        ));
    }
    let existing_slug: Option<&str> = distinct_slugs.into_iter().next();

    // ── Determine mode ───────────────────────────────────────────────────────
    //
    // Rename mode requires:
    //   - ALL batch files have slug X (no no-slug files mixed in)
    //   - The selection covers ALL files with slug X in yyyy-mm in the DB
    let use_rename_mode: bool = if let Some(old_slug) = existing_slug {
        // All batch files must carry the old slug (no day-based / no-slug files)
        let all_have_slug = batch.iter().all(|r| r.filename_slug == old_slug);
        if all_have_slug {
            // Count DB files with old_slug in yyyy-mm that are not trashed/deleted
            let pattern = format!("{batch_year}/{batch_year}-{batch_month}-{old_slug}-%");
            // Count in Rust with slug_from_path to avoid false positives (slug prefix matching).
            let confirmed_db_count: i64 = {
                let mut stmt = conn.prepare(
                    "SELECT target_path FROM media \
                     WHERE target_path LIKE ?1 \
                       AND COALESCE(status,'') NOT IN ('trashed','deleted')",
                )?;
                let count = stmt
                    .query_map(rusqlite::params![pattern], |row| row.get::<_, String>(0))?
                    .filter_map(|r| r.ok())
                    .filter(|p| slug_from_path(p) == old_slug)
                    .count() as i64;
                count
            };
            confirmed_db_count == batch.len() as i64
        } else {
            false
        }
    } else {
        false
    };

    // ── RENAME MODE ──────────────────────────────────────────────────────────
    if use_rename_mode {
        let old_slug = existing_slug.unwrap();
        let mut stats = SlugifyBatchStats {
            mode: SlugifyBatchMode::Rename,
            fixed: 0,
            skipped: 0,
            errors: Vec::new(),
        };
        for (i, rec) in batch.iter().enumerate() {
            on_progress(i, &rec.id);
            let new_path = replace_slug_in_path(&rec.target_path, new_slug);
            let new_path = match new_path {
                Some(p) => p,
                None => {
                    stats.errors.push((
                        rec.id.clone(),
                        format!("slugify: cannot parse path: {}", rec.target_path),
                    ));
                    continue;
                }
            };
            if new_path == rec.target_path && old_slug == new_slug {
                stats.skipped += 1;
                continue;
            }
            let old_abs = Path::new(target_root).join(&rec.target_path);
            let new_abs = Path::new(target_root).join(&new_path);
            if old_abs != new_abs && new_abs.exists() {
                stats.errors.push((
                    rec.id.clone(),
                    format!("slugify: target already exists: {}", new_abs.display()),
                ));
                continue;
            }
            if let Err(e) = rename_file_rel(Path::new(target_root), &rec.target_path, &new_path) {
                stats.errors.push((rec.id.clone(), e.to_string()));
                continue;
            }
            if let Err(e) = conn.execute(
                "UPDATE media SET target_path = ?1 WHERE id = ?2",
                rusqlite::params![new_path, rec.id],
            ) {
                stats.errors.push((rec.id.clone(), e.to_string()));
                continue;
            }
            stats.fixed += 1;
        }
        return Ok(stats);
    }

    // ── ASSIGN MODE ──────────────────────────────────────────────────────────
    //
    // Step A: For each old slug that loses files, recount remaining files.
    // Step B: Merge existing target-slug files + batch files → sort → assign 0001..N.

    let batch_ids: HashSet<&str> = batch.iter().map(|r| r.id.as_str()).collect();

    // Collect old slugs that need recount (files remaining after partial selection).
    let slugs_to_recount: HashSet<String> = batch
        .iter()
        .filter(|r| !r.filename_slug.is_empty() && r.filename_slug != new_slug)
        .map(|r| r.filename_slug.clone())
        .collect();

    // Pre-compute recount assignments for each old slug's remaining files.
    // Map: file_id -> new_target_path
    let mut recount_assignments: HashMap<String, String> = HashMap::new();
    for old_slug in &slugs_to_recount {
        let pattern = format!("{batch_year}/{batch_year}-{batch_month}-{old_slug}-%");
        let mut stmt = conn.prepare(
            "SELECT id, target_path, COALESCE(os_date,''), COALESCE(source_path,'') \
             FROM media \
             WHERE target_path LIKE ?1 \
               AND COALESCE(status,'') NOT IN ('trashed','deleted')",
        )?;
        let remaining: Vec<(String, String, String, String)> = stmt
            .query_map(rusqlite::params![pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter(|(id, path, _, _)| {
                slug_from_path(path) == old_slug.as_str() && !batch_ids.contains(id.as_str())
            })
            .collect();

        if remaining.is_empty() {
            continue; // Entire slug was selected; nothing to recount.
        }

        // Sort remaining files by os_date, source_basename, target_path.
        let mut remaining = remaining;
        remaining.sort_by(|(_, pa, oa, sa), (_, pb, ob, sb)| {
            oa.cmp(ob)
                .then(
                    Path::new(sa)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .cmp(
                            Path::new(sb)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(""),
                        ),
                )
                .then(pa.cmp(pb))
        });

        // Assign counters 0001..N, skipping caption files (they don't have slug_cnt).
        let mut counter: u32 = 0;
        for (id, path, _, _) in &remaining {
            let caps = path_re().captures(path);
            if caps.as_ref().and_then(|c| c.name("slug_cap")).is_some() {
                // Caption file: preserve path as-is (slug renaming not needed).
                continue;
            }
            if caps.as_ref().and_then(|c| c.name("slug_cnt")).is_none() {
                // Not a slug-format file; skip.
                continue;
            }
            counter += 1;
            let new_path = build_slug_path(path, old_slug, counter);
            if let Some(p) = new_path {
                recount_assignments.insert(id.clone(), p);
            }
        }
    }

    // Build the target group: existing DB files with new_slug in yyyy-mm + batch files.
    #[derive(Debug)]
    struct TargetRec {
        id: String,
        target_path: String,
        year: String,
        month: String,
        day: String,
        caption_slug: String,
        ext: String,
        os_date: String,
        source_basename: String,
        from_batch: bool,
    }

    let mut target_group: Vec<TargetRec> = Vec::new();

    // Add existing DB files already in new_slug (not in batch).
    let pattern = format!("{batch_year}/{batch_year}-{batch_month}-{new_slug}-%");
    {
        let mut stmt = conn.prepare(
            "SELECT id, target_path, COALESCE(caption_slug,''), COALESCE(ext,''), \
                    COALESCE(os_date,''), COALESCE(source_path,''), COALESCE(derived_date,'') \
             FROM media \
             WHERE target_path LIKE ?1 \
               AND COALESCE(status,'') NOT IN ('trashed','deleted')",
        )?;
        let rows: Vec<_> = stmt
            .query_map(rusqlite::params![pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        for (id, path, cs, ex, od, sp, dd) in rows {
            if slug_from_path(&path) != new_slug || batch_ids.contains(id.as_str()) {
                continue;
            }
            let caps = path_re().captures(&path);
            let year = caps
                .as_ref()
                .and_then(|c| c.name("year"))
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| dd.get(..4).unwrap_or("").to_string());
            let month = caps
                .as_ref()
                .and_then(|c| c.name("month"))
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| dd.get(5..7).unwrap_or("").to_string());
            let day = caps
                .as_ref()
                .and_then(|c| c.name("day"))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let source_basename = Path::new(&sp)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            target_group.push(TargetRec {
                id,
                target_path: path,
                year,
                month,
                day,
                caption_slug: cs,
                ext: ex,
                os_date: od,
                source_basename,
                from_batch: false,
            });
        }
    }

    // Add batch files.
    for r in &batch {
        target_group.push(TargetRec {
            id: r.id.clone(),
            target_path: r.target_path.clone(),
            year: r.year.clone(),
            month: r.month.clone(),
            day: r.day.clone(),
            caption_slug: r.caption_slug.clone(),
            ext: r.ext.clone(),
            os_date: r.os_date.clone(),
            source_basename: r.source_basename.clone(),
            from_batch: true,
        });
    }

    // Sort by os_date, source_basename, target_path.
    target_group.sort_by(|a, b| {
        a.os_date
            .cmp(&b.os_date)
            .then(a.source_basename.cmp(&b.source_basename))
            .then(a.target_path.cmp(&b.target_path))
    });

    // Pre-compute new paths for target group (non-caption first, then caption).
    // Maps: file_id -> (new_target_path, new_counter)
    let mut assign_new_paths: HashMap<String, (String, Option<u32>)> = HashMap::new();
    let mut counter: u32 = 0;
    // Seen paths for caption collision avoidance (disk-based).
    let mut seen_new_paths: HashSet<String> = HashSet::new();
    // Existing paths for batch files being replaced (exclude from collision check).
    let skip_disk_paths: HashSet<String> =
        target_group.iter().map(|r| r.target_path.clone()).collect();

    for tr in &target_group {
        if !tr.caption_slug.is_empty() {
            // Caption file: path is yyyy/yyyy-mm-dd-{new_slug}-{caption}.{ext}
            let day = if tr.day.is_empty() {
                "01".to_string()
            } else {
                tr.day.clone()
            };
            let date_prefix = format!("{}-{}-{}-{new_slug}", tr.year, tr.month, day);
            let base_path = format!("{}/{}-{}.{}", tr.year, date_prefix, tr.caption_slug, tr.ext);
            let new_path = if !seen_new_paths.contains(&base_path)
                && (skip_disk_paths.contains(&base_path)
                    || !Path::new(target_root).join(&base_path).exists())
            {
                base_path.clone()
            } else {
                // Find collision-free suffix.
                let mut n = 2u32;
                loop {
                    let candidate = format!(
                        "{}/{}-{}-{}.{}",
                        tr.year, date_prefix, tr.caption_slug, n, tr.ext
                    );
                    if !seen_new_paths.contains(&candidate)
                        && (skip_disk_paths.contains(&candidate)
                            || !Path::new(target_root).join(&candidate).exists())
                    {
                        break candidate;
                    }
                    n += 1;
                }
            };
            seen_new_paths.insert(new_path.clone());
            assign_new_paths.insert(tr.id.clone(), (new_path, None));
        } else {
            // Non-caption: assign sequential counter.
            counter += 1;
            let day = if tr.day.is_empty() {
                "01".to_string()
            } else {
                tr.day.clone()
            };
            let new_path = format!(
                "{}/{}-{}-{}-{:04}.{}",
                tr.year, tr.year, tr.month, new_slug, counter, tr.ext
            );
            seen_new_paths.insert(new_path.clone());
            assign_new_paths.insert(tr.id.clone(), (new_path, Some(counter)));
            let _ = day; // day used implicitly via struct; suppress warning
        }
    }

    // ── Execute: recount old slugs first, then assign new slug ──────────────
    let mut stats = SlugifyBatchStats {
        mode: SlugifyBatchMode::Assign,
        fixed: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    // Recount remaining files of old slugs.
    for (id, new_path) in &recount_assignments {
        // Find original path.
        let orig_path: String = conn.query_row(
            "SELECT target_path FROM media WHERE id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )?;
        let old_abs = Path::new(target_root).join(&orig_path);
        let new_abs = Path::new(target_root).join(new_path);
        if old_abs != new_abs {
            if new_abs.exists() {
                stats.errors.push((
                    id.clone(),
                    format!(
                        "slugify recount: target already exists: {}",
                        new_abs.display()
                    ),
                ));
                continue;
            }
            if let Err(e) = rename_file_rel(Path::new(target_root), &orig_path, new_path) {
                stats.errors.push((id.clone(), e.to_string()));
                continue;
            }
        }
        let _ = conn.execute(
            "UPDATE media SET target_path = ?1 WHERE id = ?2",
            rusqlite::params![new_path, id],
        );
    }

    // Process target group (batch + existing new-slug files).
    for (i, tr) in target_group.iter().enumerate() {
        if tr.from_batch {
            on_progress(i, &tr.id);
        }
        let (new_path, new_counter) = match assign_new_paths.get(&tr.id) {
            Some(v) => v,
            None => {
                stats
                    .errors
                    .push((tr.id.clone(), "slugify: no assignment computed".into()));
                continue;
            }
        };
        let old_abs = Path::new(target_root).join(&tr.target_path);
        let new_abs = Path::new(target_root).join(new_path);
        if old_abs == new_abs {
            if !tr.from_batch {
                // Existing new-slug file already in correct position; skip silently.
            } else {
                stats.skipped += 1;
            }
            continue;
        }
        if new_abs.exists() {
            stats.errors.push((
                tr.id.clone(),
                format!("slugify: target already exists: {}", new_abs.display()),
            ));
            continue;
        }
        if let Err(e) = rename_file_rel(Path::new(target_root), &tr.target_path, new_path) {
            stats.errors.push((tr.id.clone(), e.to_string()));
            continue;
        }
        if let Err(e) = conn.execute(
            "UPDATE media SET target_path = ?1, counter = ?2 WHERE id = ?3",
            rusqlite::params![new_path, new_counter, tr.id],
        ) {
            stats.errors.push((tr.id.clone(), e.to_string()));
            continue;
        }
        if tr.from_batch {
            stats.fixed += 1;
        }
    }

    Ok(stats)
}

/// Replace the slug token in a slug-format `target_path` with `new_slug`.
/// Returns `None` if the path is not a slug-format path.
fn replace_slug_in_path(path: &str, new_slug: &str) -> Option<String> {
    let caps = path_re().captures(path)?;
    let year = caps.name("year")?.as_str();
    let month = caps.name("month")?.as_str();
    caps.name("slug")?; // must be a slug-format path
    let slug_cnt = caps.name("slug_cnt")?.as_str();
    let ext = caps.name("ext")?.as_str();
    Some(match caps.name("slug_cap") {
        Some(cap) => format!(
            "{year}/{year}-{month}-{new_slug}-{slug_cnt}-{}.{ext}",
            cap.as_str()
        ),
        None => format!("{year}/{year}-{month}-{new_slug}-{slug_cnt}.{ext}"),
    })
}

/// Build a new path for a slug-format file with a given counter.
/// Returns `None` if path is not slug-format.
fn build_slug_path(path: &str, _old_slug: &str, counter: u32) -> Option<String> {
    let caps = path_re().captures(path)?;
    let year = caps.name("year")?.as_str();
    let month = caps.name("month")?.as_str();
    let slug = caps.name("slug")?.as_str();
    let ext = caps.name("ext")?.as_str();
    Some(match caps.name("slug_cap") {
        Some(cap) => format!(
            "{year}/{year}-{month}-{slug}-{counter:04}-{}.{ext}",
            cap.as_str()
        ),
        None => format!("{year}/{year}-{month}-{slug}-{counter:04}.{ext}"),
    })
}

///
/// Re-derives the best timestamp using the same priority logic as the import
/// execute phase ([`crate::import::best_mtime`]):
/// 1. Full timestamp encoded in the original source filename.
/// 2. `derived_date` at noon UTC (source mtime unavailable for repair).
///
/// Returns `Ok(true)` when the mtime was updated, `Ok(false)` when the file
/// was skipped (no `derived_date` stored, or the target file does not exist on
/// disk — both are non-fatal).
pub fn fix_os_time(conn: &Connection, target_root: &str, file_id: &str) -> Result<bool> {
    use std::path::Path;

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
pub fn trash_files(conn: &Connection, media_ids: &[String]) -> Result<usize> {
    if media_ids.is_empty() {
        return Ok(0);
    }
    let ph: String = std::iter::repeat_n("?", media_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let updated = conn.execute(
        &format!("UPDATE media SET status = 'trashed' WHERE id IN ({ph})"),
        rusqlite::params_from_iter(media_ids),
    )?;
    Ok(updated)
}

/// Restore trashed media files to normal (`status = 'moved'`).
///
/// Returns the number of rows updated.
pub fn keep_files(conn: &Connection, media_ids: &[String]) -> Result<usize> {
    if media_ids.is_empty() {
        return Ok(0);
    }
    let ph: String = std::iter::repeat_n("?", media_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let updated = conn.execute(
        &format!("UPDATE media SET status = 'moved' WHERE id IN ({ph})"),
        rusqlite::params_from_iter(media_ids),
    )?;
    Ok(updated)
}

/// Load up to `limit` trashed files (`status = 'trashed'`), ordered by `target_path`.
pub fn load_trashed_files(conn: &Connection, limit: usize) -> Result<Vec<MediaFile>> {
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
            tags_str
                .split('\x1f')
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
    files.sort_by(|a, b| {
        let ka = path_sort_key(&a.target_path);
        let kb = path_sort_key(&b.target_path);
        ka.0.cmp(kb.0).then(ka.1.cmp(&kb.1))
    });
    Ok(files)
}

/// Delete the given trashed files from the filesystem and mark them `status = 'deleted'` in DB.
///
/// Each file is processed independently; FS errors are non-fatal and counted.
/// Returns `(deleted, errors)`.
pub fn delete_trashed_from_fs(
    conn: &Connection,
    target_root: &str,
    media_ids: &[String],
) -> Result<(usize, usize)> {
    if media_ids.is_empty() {
        return Ok((0, 0));
    }

    let ph: String = std::iter::repeat_n("?", media_ids.len())
        .collect::<Vec<_>>()
        .join(",");

    let target_paths: Vec<(String, String)> = {
        let sql = format!("SELECT id, target_path FROM media WHERE id IN ({ph})");
        let mut stmt = conn.prepare(&sql)?;
        let result: Vec<(String, String)> = stmt
            .query_map(rusqlite::params_from_iter(media_ids), |row| {
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
pub fn load_recent_import_source_dirs(conn: &Connection) -> Result<Vec<String>> {
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
    // `tag_index` maps tag_name → index into `groups` for O(1) lookup.
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut tag_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let tag_name: String = row.get(0)?;
        let source_path: String = row.get(2)?;
        if let Some(&idx) = tag_index.get(&tag_name) {
            groups[idx].1.push(source_path);
        } else {
            let idx = groups.len();
            tag_index.insert(tag_name.clone(), idx);
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
pub fn count_trashed(conn: &Connection) -> Result<usize> {
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
        assert_eq!(extract_hms_from_str("2022-04-18 00:00:00"), Some((0, 0, 0)));
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

        let result = fix_date(&conn, dir.to_str().unwrap(), "id1", "2023-06-15");
        assert!(result.is_ok(), "fix_date should succeed: {:?}", result);

        // Old file gone, new file present.
        assert!(!old_file.exists(), "old file should be gone after rename");
        assert!(
            new_year_dir.join(new_name).exists(),
            "new file should exist"
        );

        // DB should reflect new path and date.
        let (tp, dd, od): (String, String, String) = conn
            .query_row(
                "SELECT target_path, derived_date, os_date FROM media WHERE id='id1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(tp, "2023/2023-06-15-0001.jpg");
        assert_eq!(dd, "2023-06-15");
        assert!(
            od.starts_with("2023-06-15"),
            "os_date should start with new date, got {od}"
        );

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

        // File does not exist on disk — fix_date should still update the DB.
        let result = fix_date(&conn, dir.to_str().unwrap(), "id2", "2023-06-15");
        assert!(
            result.is_ok(),
            "fix_date on absent file should succeed: {:?}",
            result
        );

        let (tp, dd): (String, String) = conn
            .query_row(
                "SELECT target_path, derived_date FROM media WHERE id='id2'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
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

    /// `load_files` must return `moved` and `trashed` rows but never `deleted` ones.
    #[test]
    fn load_files_excludes_deleted_status() {
        use rusqlite::Connection;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("mex_load_files_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("mex.db");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                id TEXT PRIMARY KEY,
                target_path TEXT,
                derived_date TEXT,
                ext TEXT,
                os_date TEXT,
                derived_slug TEXT,
                caption_slug TEXT,
                source_path TEXT,
                status TEXT,
                missing_on_disk INTEGER DEFAULT 0
             );
             CREATE TABLE media_tags (media_id TEXT, tag_id TEXT);
             CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT, type TEXT);
             INSERT INTO media VALUES ('m1','2024/moved.jpg','2024-01-01','jpg','','','','','moved',0);
             INSERT INTO media VALUES ('m2','2024/trashed.jpg','2024-01-02','jpg','','','','','trashed',0);
             INSERT INTO media VALUES ('m3','2024/deleted.jpg','2024-01-03','jpg','','','','','deleted',0);",
        ).unwrap();

        let files = load_files(&conn).unwrap();
        let ids: Vec<&str> = files.iter().map(|f| f.id.as_str()).collect();

        assert!(
            ids.contains(&"m1"),
            "moved file must be in load_files result"
        );
        assert!(
            ids.contains(&"m2"),
            "trashed file must be in load_files result"
        );
        assert!(
            !ids.contains(&"m3"),
            "deleted file must NOT be in load_files result"
        );

        fs::remove_dir_all(&dir).ok();
    }

    // ── fix_caption tests ──────────────────────────────────────────────────

    fn make_fix_caption_db(dir: &std::path::Path) -> rusqlite::Connection {
        use rusqlite::Connection;
        let db_path = dir.join("mex.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                id          TEXT PRIMARY KEY,
                target_path TEXT,
                caption_slug TEXT,
                counter     INTEGER,
                status      TEXT
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn fix_caption_adds_caption_no_collision() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("mex_fc_add_{}", std::process::id()));
        let yr = dir.join("2025");
        fs::create_dir_all(&yr).unwrap();
        let old_file = yr.join("2025-11-30-0001.jpg");
        fs::write(&old_file, b"data").unwrap();

        let conn = make_fix_caption_db(&dir);
        conn.execute(
            "INSERT INTO media VALUES ('f1','2025/2025-11-30-0001.jpg',NULL,1,'moved')",
            [],
        )
        .unwrap();

        fix_caption(&conn, dir.to_str().unwrap(), "f1", "chisel").unwrap();

        let tp: String = conn
            .query_row("SELECT target_path FROM media WHERE id='f1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(tp, "2025/2025-11-30-chisel.jpg");
        assert!(!old_file.exists(), "old file should be renamed");
        assert!(
            yr.join("2025-11-30-chisel.jpg").exists(),
            "new file must exist"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fix_caption_removes_caption_no_collision() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("mex_fc_rm_{}", std::process::id()));
        let yr = dir.join("2025");
        fs::create_dir_all(&yr).unwrap();
        fs::write(yr.join("2025-11-30-chisel.jpg"), b"data").unwrap();

        let conn = make_fix_caption_db(&dir);
        conn.execute(
            "INSERT INTO media VALUES ('f1','2025/2025-11-30-chisel.jpg','chisel',NULL,'moved')",
            [],
        )
        .unwrap();

        fix_caption(&conn, dir.to_str().unwrap(), "f1", "").unwrap();

        let tp: String = conn
            .query_row("SELECT target_path FROM media WHERE id='f1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(tp, "2025/2025-11-30-0001.jpg");
        assert!(!yr.join("2025-11-30-chisel.jpg").exists());
        assert!(yr.join("2025-11-30-0001.jpg").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fix_caption_day_collision_precomputes_counter() {
        // The plain caption path and counter-2 through counter-3 are all occupied;
        // fix_caption must jump directly to counter 4 without looping per-iteration.
        use std::fs;
        let dir = std::env::temp_dir().join(format!("mex_fc_coll_{}", std::process::id()));
        let yr = dir.join("2025");
        fs::create_dir_all(&yr).unwrap();
        // File being edited
        fs::write(yr.join("2025-11-30-0001.jpg"), b"me").unwrap();
        // Occupying plain and counter-2 on disk
        fs::write(yr.join("2025-11-30-chisel.jpg"), b"a").unwrap();
        fs::write(yr.join("2025-11-30-chisel-2.jpg"), b"b").unwrap();

        let conn = make_fix_caption_db(&dir);
        conn.execute(
            "INSERT INTO media VALUES ('f1','2025/2025-11-30-0001.jpg',NULL,1,'moved')",
            [],
        )
        .unwrap();
        // Another file with counter-3 in DB (not on disk — exercises the DB path)
        conn.execute(
            "INSERT INTO media VALUES ('f2','2025/2025-11-30-chisel-3.jpg','chisel',3,'moved')",
            [],
        )
        .unwrap();

        fix_caption(&conn, dir.to_str().unwrap(), "f1", "chisel").unwrap();

        let tp: String = conn
            .query_row("SELECT target_path FROM media WHERE id='f1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        // max counter: FS max = 2 (chisel-2.jpg), DB max = 3 (f2) → next = 4
        assert_eq!(tp, "2025/2025-11-30-chisel-4.jpg");
        assert!(!yr.join("2025-11-30-0001.jpg").exists());
        assert!(yr.join("2025-11-30-chisel-4.jpg").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fix_caption_slug_collision_precomputes_counter() {
        // Slug-type: removing caption from christmas-0001-old.jpg would produce
        // christmas-0001.jpg, but that path is occupied; counter-0002 also exists on
        // disk, so fix_caption must jump to 0003 via pre-computed max.
        use std::fs;
        let dir = std::env::temp_dir().join(format!("mex_fc_slug_coll_{}", std::process::id()));
        let yr = dir.join("2025");
        fs::create_dir_all(&yr).unwrap();
        // File being edited (slug_cnt=0001, caption=old)
        fs::write(yr.join("2025-11-christmas-0001-old.jpg"), b"me").unwrap();
        // Block the base target (0001) and one more counter (0002)
        fs::write(yr.join("2025-11-christmas-0001.jpg"), b"blocked").unwrap();
        fs::write(yr.join("2025-11-christmas-0002.jpg"), b"other").unwrap();

        let conn = make_fix_caption_db(&dir);
        conn.execute(
            "INSERT INTO media VALUES ('f1','2025/2025-11-christmas-0001-old.jpg','old',1,'moved')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO media VALUES ('f2','2025/2025-11-christmas-0002.jpg',NULL,2,'moved')",
            [],
        )
        .unwrap();

        fix_caption(&conn, dir.to_str().unwrap(), "f1", "").unwrap();

        let tp: String = conn
            .query_row("SELECT target_path FROM media WHERE id='f1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        // FS max = 2 (0002.jpg), DB max = 2 (f2, counter=2), f1 excluded → next = 3
        assert_eq!(tp, "2025/2025-11-christmas-0003.jpg");
        assert!(!yr.join("2025-11-christmas-0001-old.jpg").exists());
        assert!(yr.join("2025-11-christmas-0003.jpg").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fix_caption_clears_counter_for_plain_caption() {
        // A day-counter file (counter=1) converted to a plain caption must
        // have counter set to NULL in the DB so that deslugify's db_max query
        // does not see a stale "1" and skip 0001.
        use std::fs;
        let dir = std::env::temp_dir().join(format!("mex_fc_clr_{}", std::process::id()));
        let yr = dir.join("2026");
        fs::create_dir_all(&yr).unwrap();
        fs::write(yr.join("2026-01-04-0001.jpg"), b"data").unwrap();

        let conn = make_fix_caption_db(&dir);
        conn.execute(
            "INSERT INTO media VALUES ('f1','2026/2026-01-04-0001.jpg',NULL,1,'moved')",
            [],
        )
        .unwrap();

        fix_caption(
            &conn,
            dir.to_str().unwrap(),
            "f1",
            "com-google-android-youtube",
        )
        .unwrap();

        let (tp, ctr): (String, Option<u32>) = conn
            .query_row(
                "SELECT target_path, counter FROM media WHERE id='f1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(tp, "2026/2026-01-04-com-google-android-youtube.jpg");
        assert_eq!(ctr, None, "plain caption must have counter = NULL");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fix_caption_stores_collision_counter() {
        // When the plain caption path is already taken, fix_caption falls back
        // to a collision suffix (e.g. -2).  The stored counter must equal that
        // collision number, not the old day counter.
        use std::fs;
        let dir = std::env::temp_dir().join(format!("mex_fc_coll2_{}", std::process::id()));
        let yr = dir.join("2026");
        fs::create_dir_all(&yr).unwrap();
        fs::write(yr.join("2026-01-04-0001.jpg"), b"data").unwrap();
        // Occupy the plain caption path on disk.
        fs::write(yr.join("2026-01-04-snapshot.jpg"), b"other").unwrap();

        let conn = make_fix_caption_db(&dir);
        conn.execute(
            "INSERT INTO media VALUES ('f1','2026/2026-01-04-0001.jpg',NULL,1,'moved')",
            [],
        )
        .unwrap();

        fix_caption(&conn, dir.to_str().unwrap(), "f1", "snapshot").unwrap();

        let (tp, ctr): (String, Option<u32>) = conn
            .query_row(
                "SELECT target_path, counter FROM media WHERE id='f1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(tp, "2026/2026-01-04-snapshot-2.jpg");
        assert_eq!(
            ctr,
            Some(2),
            "collision caption must store the collision counter"
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// `load_recent_import_source_dirs` must group rows by tag and return one
    /// common-prefix entry per import session, ordered by recency.
    #[test]
    fn load_recent_import_source_dirs_groups_by_tag() {
        use rusqlite::Connection;

        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT, type TEXT);
             CREATE TABLE media_tags (media_id TEXT, tag_id TEXT);
             CREATE TABLE media (
                 id TEXT PRIMARY KEY, target_path TEXT, derived_date TEXT,
                 ext TEXT, os_date TEXT, derived_slug TEXT, caption_slug TEXT,
                 source_path TEXT, status TEXT, moved_at INTEGER DEFAULT 0,
                 missing_on_disk INTEGER DEFAULT 0
             );

             -- Two import sessions: import-2 is more recent
             INSERT INTO tags VALUES ('t1', 'import-1', 'mex');
             INSERT INTO tags VALUES ('t2', 'import-2', 'mex');

             -- Session import-1: two files under /home/user/old/
             INSERT INTO media VALUES ('m1','a','2024','jpg','','','','/home/user/old/a.jpg','moved',10,0);
             INSERT INTO media VALUES ('m2','b','2024','jpg','','','','/home/user/old/b.jpg','moved',11,0);
             INSERT INTO media_tags VALUES ('m1', 't1');
             INSERT INTO media_tags VALUES ('m2', 't1');

             -- Session import-2: two files under /home/user/new/
             INSERT INTO media VALUES ('m3','c','2024','jpg','','','','/home/user/new/c.jpg','moved',20,0);
             INSERT INTO media VALUES ('m4','d','2024','jpg','','','','/home/user/new/d.jpg','moved',21,0);
             INSERT INTO media_tags VALUES ('m3', 't2');
             INSERT INTO media_tags VALUES ('m4', 't2');",
        )
        .unwrap();

        let dirs = load_recent_import_source_dirs(&conn).unwrap();

        // Most recent session comes first.
        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0], "/home/user/new");
        assert_eq!(dirs[1], "/home/user/old");
    }

    // ── slug_from_path tests ───────────────────────────────────────────────────

    #[test]
    fn slug_from_path_slug_file() {
        assert_eq!(slug_from_path("2024/2024-11-festival-0001.jpg"), "festival");
    }

    #[test]
    fn slug_from_path_slug_file_with_caption() {
        assert_eq!(
            slug_from_path("2024/2024-11-summer-0001-beach.jpg"),
            "summer"
        );
    }

    #[test]
    fn slug_from_path_day_file_returns_empty() {
        assert_eq!(slug_from_path("2024/2024-11-30-0001.jpg"), "");
    }

    #[test]
    fn slug_from_path_unknown_returns_empty() {
        assert_eq!(slug_from_path("some/random/path.jpg"), "");
    }

    // ── replace_slug_in_path tests ────────────────────────────────────────────

    #[test]
    fn replace_slug_in_path_basic() {
        assert_eq!(
            replace_slug_in_path("2024/2024-11-festival-0003.jpg", "summer"),
            Some("2024/2024-11-summer-0003.jpg".to_string())
        );
    }

    #[test]
    fn replace_slug_in_path_with_caption() {
        assert_eq!(
            replace_slug_in_path("2024/2024-11-festival-0003-beach.jpg", "summer"),
            Some("2024/2024-11-summer-0003-beach.jpg".to_string())
        );
    }

    #[test]
    fn replace_slug_in_path_day_file_returns_none() {
        assert_eq!(
            replace_slug_in_path("2024/2024-11-30-0001.jpg", "summer"),
            None
        );
    }

    // ── deslugify_batch counter tests ────────────────────────────────────────

    fn make_deslugify_db(dir: &std::path::Path) -> rusqlite::Connection {
        use rusqlite::Connection;
        let conn = Connection::open(dir.join("mex.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                id TEXT PRIMARY KEY,
                target_path TEXT,
                derived_date TEXT,
                derived_slug TEXT,
                caption_slug TEXT,
                ext TEXT,
                os_date TEXT,
                source_path TEXT,
                status TEXT,
                counter INTEGER
             );
             CREATE TABLE media_tags (media_id TEXT, tag_id INTEGER);
             CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT, type TEXT);",
        )
        .unwrap();
        conn
    }

    /// A caption-only file with a stale counter=1 in the DB must not cause
    /// deslugify_batch to start new counter assignments at 0002.
    #[test]
    fn deslugify_batch_ignores_caption_file_counter() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let yr = dir.path().join("2026");
        fs::create_dir_all(&yr).unwrap();

        // Slug file to be deslugified.
        fs::write(yr.join("2026-01-youtube-0001.jpg"), b"img").unwrap();

        // Caption file that existed before fix_caption was corrected —
        // it has counter=1 (stale) but must NOT count toward the day sequence.
        fs::write(yr.join("2026-01-04-com-google-android-youtube.jpg"), b"cap").unwrap();

        let conn = make_deslugify_db(dir.path());
        conn.execute_batch(
            "INSERT INTO media VALUES ('s1','2026/2026-01-youtube-0001.jpg','2026-01-04','youtube',NULL,'jpg','2026-01-04 10:00:00','img.jpg','moved',1);
             -- stale counter=1 on a caption file (bug we are guarding against):
             INSERT INTO media VALUES ('c1','2026/2026-01-04-com-google-android-youtube.jpg','2026-01-04',NULL,'com-google-android-youtube','jpg','2026-01-04 09:00:00','yt.jpg','moved',1);",
        ).unwrap();

        let stats = deslugify_batch(
            &conn,
            dir.path().to_str().unwrap(),
            &["s1".into()],
            |_, _| {},
        )
        .unwrap();

        assert_eq!(stats.fixed, 1);
        assert_eq!(stats.errors.len(), 0);

        let tp: String = conn
            .query_row("SELECT target_path FROM media WHERE id='s1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        // Caption file has stale counter=1 but must be ignored → new file
        // starts at 0001, not 0002.
        assert_eq!(tp, "2026/2026-01-04-0001.jpg", "counter must start at 0001");
        assert!(yr.join("2026-01-04-0001.jpg").exists());

        fs::remove_dir_all(dir).ok();
    }

    // ── slugify_batch guard tests ─────────────────────────────────────────────

    fn make_slugify_db(dir: &std::path::Path) -> rusqlite::Connection {
        use rusqlite::Connection;
        let db_path = dir.join("mex.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                id TEXT PRIMARY KEY,
                target_path TEXT,
                derived_date TEXT,
                derived_slug TEXT,
                caption_slug TEXT,
                ext TEXT,
                os_date TEXT,
                source_path TEXT,
                status TEXT,
                counter INTEGER
             );
             CREATE TABLE media_tags (media_id TEXT, tag_id INTEGER);
             CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT, type TEXT);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn slugify_batch_multi_month_guard() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let conn = make_slugify_db(dir.path());

        conn.execute_batch(
            "INSERT INTO media VALUES ('a','2024/2024-10-festival-0001.jpg','2024-10-01','festival','','jpg','','','moved',1);
             INSERT INTO media VALUES ('b','2024/2024-11-festival-0001.jpg','2024-11-01','festival','','jpg','','','moved',1);",
        ).unwrap();

        let target_root = dir.path().to_str().unwrap();
        let result = slugify_batch(
            &conn,
            target_root,
            &["a".into(), "b".into()],
            "newslug",
            |_, _| {},
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("same yyyy-mm"),
            "expected multi-month error, got: {err}"
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn slugify_batch_multi_slug_guard() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let conn = make_slugify_db(dir.path());

        conn.execute_batch(
            "INSERT INTO media VALUES ('a','2024/2024-11-alpha-0001.jpg','2024-11-01','alpha','','jpg','','','moved',1);
             INSERT INTO media VALUES ('b','2024/2024-11-beta-0001.jpg','2024-11-01','beta','','jpg','','','moved',1);",
        ).unwrap();

        let target_root = dir.path().to_str().unwrap();
        let result = slugify_batch(
            &conn,
            target_root,
            &["a".into(), "b".into()],
            "newslug",
            |_, _| {},
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("cannot mix"),
            "expected multi-slug error, got: {err}"
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn slugify_batch_assign_mode_renames_files() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let conn = make_slugify_db(dir.path());

        // Create source files on disk.
        let yr_dir = dir.path().join("2024");
        fs::create_dir_all(&yr_dir).unwrap();
        fs::write(yr_dir.join("2024-11-30-0001.jpg"), b"img1").unwrap();
        fs::write(yr_dir.join("2024-11-30-0002.jpg"), b"img2").unwrap();

        conn.execute_batch(
            "INSERT INTO media VALUES ('a','2024/2024-11-30-0001.jpg','2024-11-30','','','jpg','2024-11-30 10:00:00','a.jpg','moved',1);
             INSERT INTO media VALUES ('b','2024/2024-11-30-0002.jpg','2024-11-30','','','jpg','2024-11-30 11:00:00','b.jpg','moved',2);",
        ).unwrap();

        let target_root = dir.path().to_str().unwrap();
        let stats = slugify_batch(
            &conn,
            target_root,
            &["a".into(), "b".into()],
            "party",
            |_, _| {},
        )
        .unwrap();

        assert_eq!(stats.fixed, 2, "both files should be moved");
        assert_eq!(stats.mode, SlugifyBatchMode::Assign);

        // Verify new paths exist.
        assert!(yr_dir.join("2024-11-party-0001.jpg").exists());
        assert!(yr_dir.join("2024-11-party-0002.jpg").exists());

        fs::remove_dir_all(dir).ok();
    }
}
