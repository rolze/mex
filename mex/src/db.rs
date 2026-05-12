use anyhow::Result;
use rusqlite::Connection;

#[derive(Clone, Debug)]
pub struct MediaFile {
    pub id: String,
    pub target_path: String,
    pub derived_date: String,
    pub ext: String,
    pub tags: Vec<String>,
    pub derived_slug: String,
    pub caption_slug: String,
}

pub fn load_files(db_path: &str) -> Result<Vec<MediaFile>> {
    let conn = Connection::open(db_path)?;

    let sql = "SELECT m.id, m.target_path, COALESCE(m.derived_date,''), m.ext,
                      COALESCE(GROUP_CONCAT(t.name, ', '),''),
                      COALESCE(m.derived_slug,''), COALESCE(m.caption_slug,'')
               FROM media m
               LEFT JOIN media_tags mt ON mt.media_id = m.id
               LEFT JOIN tags t ON t.id = mt.tag_id
               WHERE m.target_path IS NOT NULL
               GROUP BY m.id
               ORDER BY m.target_path";

    let mut stmt = conn.prepare(sql)?;

    let rows = stmt.query_map([], |row| {
        let tags_str: String = row.get(4)?;
        Ok(MediaFile {
            id: row.get(0)?,
            target_path: row.get(1)?,
            derived_date: row.get(2)?,
            ext: row.get(3)?,
            tags: if tags_str.is_empty() {
                vec![]
            } else {
                tags_str.split(", ").map(|s| s.to_string()).collect()
            },
            derived_slug: row.get(5)?,
            caption_slug: row.get(6)?,
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
