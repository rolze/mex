import sqlite3
import os
import sys

def migrate(old_db_path, new_db_path):
    if not os.path.exists(old_db_path):
        print(f"Error: {old_db_path} does not exist.")
        sys.exit(1)
        
    print(f"Migrating {old_db_path} -> {new_db_path}")

    # Connect to databases
    old_db = sqlite3.connect(old_db_path)
    old_db.row_factory = sqlite3.Row
    
    # Remove new db if it exists
    if os.path.exists(new_db_path):
        os.remove(new_db_path)
        
    new_db = sqlite3.connect(new_db_path)
    
    # Create new schema
    new_db.executescript("""
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;
PRAGMA cache_size   = -65536;
PRAGMA temp_store   = memory;
PRAGMA foreign_keys = ON;

CREATE TABLE tags (
    id   INTEGER PRIMARY KEY,
    name TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    type TEXT    NOT NULL DEFAULT 'event'
) STRICT;

CREATE TABLE media (
    id               TEXT    PRIMARY KEY,
    source_path      TEXT    NOT NULL UNIQUE,
    path_stem        TEXT    UNIQUE,
    partial_hash     TEXT    NOT NULL,
    file_size        INTEGER NOT NULL,
    ext              TEXT    NOT NULL CHECK(ext LIKE '.%'),
    mex_date         TEXT    NOT NULL,
    exif_date        TEXT,
    xmp_date         TEXT,
    os_date          TEXT,
    status           TEXT    NOT NULL DEFAULT 'imported' CHECK(status IN ('imported','duplicate','trashed','deleted')),
    missing_on_disk  INTEGER NOT NULL DEFAULT 0,
    tags_packed      TEXT    NOT NULL DEFAULT '',
    tag_types_packed TEXT    NOT NULL DEFAULT ''
) STRICT;

CREATE TABLE events (
    id         INTEGER PRIMARY KEY,
    media_id   TEXT    NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    event_type TEXT    NOT NULL,
    timestamp  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TABLE media_tags (
    media_id TEXT    NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (media_id, tag_id)
) STRICT;

CREATE INDEX idx_media_path_stem    ON media(path_stem);
CREATE INDEX idx_media_status       ON media(status);
CREATE INDEX idx_media_partial_hash ON media(partial_hash);
CREATE INDEX idx_media_tags_tag     ON media_tags(tag_id);
CREATE INDEX idx_events_media       ON events(media_id, event_type);

CREATE TRIGGER media_immutable
BEFORE UPDATE ON media FOR EACH ROW
WHEN OLD.id           IS NOT NEW.id
  OR OLD.source_path  IS NOT NEW.source_path
  OR OLD.partial_hash IS NOT NEW.partial_hash
  OR OLD.exif_date    IS NOT NEW.exif_date
  OR OLD.xmp_date     IS NOT NEW.xmp_date
  OR OLD.os_date      IS NOT NEW.os_date
  OR OLD.ext          IS NOT NEW.ext
BEGIN
  SELECT RAISE(ABORT, 'media: immutable column update rejected');
END;
    """)

    # Migrate Tags
    print("Migrating tags...")
    tags = old_db.execute("SELECT id, name, type FROM tags").fetchall()
    new_db.executemany("INSERT INTO tags (id, name, type) VALUES (?, ?, ?)", 
                       [(t['id'], t['name'], t['type']) for t in tags])

    # Migrate Media
    print("Migrating media...")
    media_rows = old_db.execute("SELECT * FROM media").fetchall()
    
    # Pre-fetch tags for packing
    media_tags = old_db.execute("""
        SELECT mt.media_id, t.name, t.type 
        FROM media_tags mt 
        JOIN tags t ON mt.tag_id = t.id
    """).fetchall()
    tags_by_media = {}
    for mt in media_tags:
        tags_by_media.setdefault(mt['media_id'], []).append((mt['name'], mt['type']))

    new_media_data = []
    
    used_stems = set()
    
    for row in media_rows:
        row_keys = row.keys()
        
        # id
        m_id = row['id']
        
        # source_path
        source_path = row['source_path']
        
        # path_stem from target_path (e.g. 2022/2022-04-18-stiegenhaus.jpg)
        target_path = row['target_path'] if 'target_path' in row_keys else None
        path_stem = None
        if target_path:
            # strip directory
            basename = os.path.basename(target_path)
            # strip extension
            base_stem, old_ext = os.path.splitext(basename)
            path_stem = base_stem
            counter = 2
            while path_stem in used_stems:
                path_stem = f"{base_stem}-{counter}"
                counter += 1
            used_stems.add(path_stem)
            
            # If path_stem changed, rename file on disk
            if path_stem != base_stem:
                base_dir = os.path.dirname(old_db_path) # e.g. /srv/data/media/year
                old_full_path = os.path.join(base_dir, target_path)
                new_target_path = f"{path_stem[:4]}/{path_stem}{old_ext}"
                new_full_path = os.path.join(base_dir, new_target_path)
                if os.path.exists(old_full_path):
                    print(f"Renaming on disk due to stem collision: {old_full_path} -> {new_full_path}")
                    os.rename(old_full_path, new_full_path)
            
        # partial_hash
        partial_hash = row['partial_hash'] if 'partial_hash' in row_keys else None
        if not partial_hash:
            partial_hash = "" # NOT NULL constraint
            
        # file_size
        file_size = row['file_size'] if 'file_size' in row_keys else None
        if file_size is None:
            file_size = 0
            
        # ext
        ext = row['ext'] if 'ext' in row_keys else None
        if not ext:
            ext = os.path.splitext(source_path)[1]
            if not ext:
                ext = ".unknown"
        if not ext.startswith("."):
            ext = "." + ext
            
        # mex_date
        derived_date = row['derived_date'] if 'derived_date' in row_keys else None
        old_os_date = row['os_date'] if 'os_date' in row_keys else None
        exif_date = row['exif_date'] if 'exif_date' in row_keys else None
        
        mex_date = None
        
        # Try old os_date
        if old_os_date and derived_date and old_os_date.startswith(derived_date):
            mex_date = old_os_date
        
        # Try exif_date
        if not mex_date and exif_date and derived_date:
            exif_date_norm = exif_date.replace(':', '-', 2).replace('/', '-', 2)
            if exif_date_norm.startswith(derived_date):
                mex_date = exif_date_norm
                
        # Fallback to derived_date noon UTC
        if not mex_date:
            if derived_date:
                mex_date = f"{derived_date} 12:00:00"
            else:
                mex_date = "1970-01-01 12:00:00"
                
        # Format mex_date YYYY-MM-DD HH:MM:SS
        mex_date = mex_date.replace('/', '-').replace(':', '-', 2)
        
        # xmp_date
        xmp_date = row['xmp_date'] if 'xmp_date' in row_keys else None
        if not xmp_date and 'has_xmp_sidecar' in row_keys and row['has_xmp_sidecar']:
            xmp_date = mex_date # Best guess if missing
            
        # os_date
        new_os_date = row['orig_os_date'] if 'orig_os_date' in row_keys else None
        
        # status mapping
        old_status = row['status'] if 'status' in row_keys else None
        if old_status == 'moved' or old_status == 'pending':
            status = 'imported'
        else:
            status = old_status
            
        # Ensure status is valid
        if status not in ('imported', 'duplicate', 'trashed', 'deleted'):
            status = 'imported'
            
        missing_on_disk = row['missing_on_disk'] if 'missing_on_disk' in row_keys else None
        if missing_on_disk is None:
            missing_on_disk = 0
            
        # tags_packed, tag_types_packed
        mtags = tags_by_media.get(m_id, [])
        tags_packed = chr(31).join(t[0] for t in mtags)
        tag_types_packed = chr(31).join(t[1] for t in mtags)
        
        new_media_data.append((
            m_id, source_path, path_stem, partial_hash, file_size, ext,
            mex_date, exif_date, xmp_date, new_os_date, status, missing_on_disk,
            tags_packed, tag_types_packed
        ))
        
    new_db.executemany("""
        INSERT INTO media (id, source_path, path_stem, partial_hash, file_size, ext,
                           mex_date, exif_date, xmp_date, os_date, status, missing_on_disk,
                           tags_packed, tag_types_packed)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    """, new_media_data)
    
    # Migrate media_tags
    print("Migrating media_tags...")
    mt_rows = old_db.execute("SELECT media_id, tag_id FROM media_tags").fetchall()
    new_db.executemany("INSERT INTO media_tags (media_id, tag_id) VALUES (?, ?)",
                       [(mt['media_id'], mt['tag_id']) for mt in mt_rows])
                       
    # Migrate events (scanned_at, moved_at)
    print("Migrating events...")
    old_tables = [r['name'] for r in old_db.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()]
    events_data = []
    
    if 'events' in old_tables:
        old_events = old_db.execute("SELECT media_id, event_type, timestamp FROM events").fetchall()
        for oe in old_events:
            events_data.append((oe['media_id'], oe['event_type'], oe['timestamp']))
            
    for row in media_rows:
        row_keys = row.keys()
        m_id = row['id']
        scanned_at = row['scanned_at'] if 'scanned_at' in row_keys else None
        moved_at = row['moved_at'] if 'moved_at' in row_keys else None
        
        if scanned_at:
            events_data.append((m_id, 'scanned', scanned_at))
        if moved_at:
            events_data.append((m_id, 'imported', moved_at))
            
    # Insert events and ignore duplicates if any
    new_db.executemany("INSERT OR IGNORE INTO events (media_id, event_type, timestamp) VALUES (?, ?, ?)",
                       events_data)
                       
    new_db.commit()
    print("Migration complete!")

if __name__ == '__main__':
    migrate('/srv/data/media/year/mex.db', '/srv/data/media/year/mex_v1.db')
