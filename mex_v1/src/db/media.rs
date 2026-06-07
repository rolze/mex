use crate::domain::media::{MediaItem, Status};
use rusqlite::{Connection, Result};

/// Loads all active files (status != 'deleted') into memory.
/// Uses the single table scan approach described in DATABASE.md for performance.
pub fn load_files(conn: &Connection) -> Result<Vec<MediaItem>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, path_stem, ext, mex_date,
               tags_packed, tag_types_packed,
               os_date, source_path, status, missing_on_disk
        FROM media
        WHERE path_stem IS NOT NULL AND status != 'deleted'
        ORDER BY path_stem
        ",
    )?;

    let iter = stmt.query_map([], |row| {
        Ok(MediaItem {
            id: row.get(0)?,
            path_stem: row.get(1)?,
            ext: row.get(2)?,
            mex_date: row.get(3)?,
            tags_packed: row.get(4)?,
            tag_types_packed: row.get(5)?,
            os_date: row.get(6)?,
            source_path: row.get(7)?,
            status: Status::from_str(&row.get::<_, String>(8)?),
            missing_on_disk: row.get::<_, i64>(9)? != 0,
            caption: None,
        })
    })?;

    let mut items = Vec::new();
    for item in iter {
        items.push(item?);
    }
    Ok(items)
}

pub fn update_status(conn: &Connection, ids: &[String], status: Status) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    // SQLite limits parameters, but we just use a transaction and prepare a statement
    let mut stmt = conn.prepare("UPDATE media SET status = ?1 WHERE id = ?2")?;
    let status_str = status.as_str();

    // Begin transaction for performance
    conn.execute("BEGIN TRANSACTION", [])?;
    for id in ids {
        stmt.execute([status_str, id])?;
    }
    conn.execute("COMMIT", [])?;

    Ok(())
}

pub fn update_caption(conn: &Connection, ids: &[String], caption: Option<&str>) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "UPDATE media SET caption = ?1 WHERE id IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&query)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&caption];
    for id in ids {
        params.push(id);
    }
    stmt.execute(rusqlite::params_from_iter(params))?;
    Ok(())
}
