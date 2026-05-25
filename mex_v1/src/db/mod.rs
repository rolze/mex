use std::path::Path;
use anyhow::{Context, Result};
use rusqlite::Connection;

pub mod media;
pub mod tags;

pub fn init_db<P: AsRef<Path>>(path: P) -> Result<Connection> {
    let conn = Connection::open(path).context("Failed to open SQLite connection")?;

    // PRAGMAs recommended in DATABASE.md
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -65536;
        PRAGMA temp_store = memory;
        PRAGMA foreign_keys = ON;
        ",
    )?;

    // Create schema
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tags (
            id   INTEGER PRIMARY KEY,
            name TEXT    NOT NULL UNIQUE COLLATE NOCASE,
            type TEXT    NOT NULL DEFAULT 'event'
        ) STRICT;

        CREATE TABLE IF NOT EXISTS media (
            id               TEXT    PRIMARY KEY,
            source_path      TEXT    NOT NULL UNIQUE,
            path_stem        TEXT    UNIQUE,
            partial_hash     TEXT    NOT NULL,
            file_size        INTEGER NOT NULL,
            ext              TEXT    NOT NULL CHECK(ext LIKE '.%'),
            derived_date     TEXT    NOT NULL,
            orig_exif_date   TEXT,
            orig_xmp_date    TEXT,
            orig_os_date     TEXT,
            status           TEXT    NOT NULL DEFAULT 'imported'
                                     CHECK(status IN ('imported','normal','duplicate','trashed','deleted')),
            missing_on_disk  INTEGER NOT NULL DEFAULT 0,
            tags_packed      TEXT    NOT NULL DEFAULT '',
            tag_types_packed TEXT    NOT NULL DEFAULT '',
            caption          TEXT
        ) STRICT;

        CREATE TABLE IF NOT EXISTS events (
            id         INTEGER PRIMARY KEY,
            media_id   TEXT    NOT NULL REFERENCES media(id) ON DELETE CASCADE,
            event_type TEXT    NOT NULL,
            timestamp  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        ) STRICT;

        CREATE TABLE IF NOT EXISTS media_tags (
            media_id TEXT    NOT NULL REFERENCES media(id) ON DELETE CASCADE,
            tag_id   INTEGER NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
            PRIMARY KEY (media_id, tag_id)
        ) STRICT;

        CREATE INDEX IF NOT EXISTS idx_media_path_stem    ON media(path_stem);
        CREATE INDEX IF NOT EXISTS idx_media_status       ON media(status);
        CREATE INDEX IF NOT EXISTS idx_media_partial_hash ON media(partial_hash);
        CREATE INDEX IF NOT EXISTS idx_media_tags_tag     ON media_tags(tag_id);
        CREATE INDEX IF NOT EXISTS idx_events_media       ON events(media_id, event_type);
        ",
    ).context("Failed to initialize database schema")?;

    Ok(conn)
}
