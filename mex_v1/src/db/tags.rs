use crate::domain::tag::Tag;
use rusqlite::{Connection, Result};

#[allow(dead_code)]
pub fn load_all_tags(conn: &Connection) -> Result<Vec<Tag>> {
    let mut stmt = conn.prepare("SELECT id, name, type FROM tags ORDER BY name")?;
    let iter = stmt.query_map([], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            type_: row.get(2)?,
        })
    })?;

    let mut tags = Vec::new();
    for tag in iter {
        tags.push(tag?);
    }
    Ok(tags)
}
