import sqlite3

def fix_partial_hashes():
    old_db_path = '/srv/data/media/year/mex.db'
    new_db_path = '/srv/data/media/year/mex_v1.db'
    
    # 1. Read mappings from old DB
    old_db = sqlite3.connect(old_db_path)
    old_db.row_factory = sqlite3.Row
    
    # Create mapping: content_hash -> partial_hash
    mapping = {}
    rows_with_hash = old_db.execute("SELECT content_hash, partial_hash FROM media WHERE partial_hash IS NOT NULL").fetchall()
    for row in rows_with_hash:
        ch = row['content_hash']
        ph = row['partial_hash']
        if ch and ph:
            mapping[ch] = ph
            
    # Find IDs that need fixing
    needs_fix = []
    # In mex_v1, we set partial_hash = '' for missing ones. Let's find those in old DB
    missing_rows = old_db.execute("SELECT id, content_hash FROM media WHERE partial_hash IS NULL").fetchall()
    for row in missing_rows:
        m_id = row['id']
        ch = row['content_hash']
        if ch in mapping:
            needs_fix.append((mapping[ch], m_id))
            
    if not needs_fix:
        print("No hashes need fixing.")
        return
        
    print(f"Found {len(needs_fix)} duplicate entries that need partial_hash refilled.")
    
    # 2. Update new DB
    new_db = sqlite3.connect(new_db_path)
    
    try:
        new_db.execute("BEGIN;")
        new_db.execute("DROP TRIGGER IF EXISTS media_immutable;")
        
        new_db.executemany("UPDATE media SET partial_hash = ? WHERE id = ?;", needs_fix)
        
        new_db.execute("""
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
        
        new_db.execute("COMMIT;")
        print("Successfully updated mex_v1.db.")
    except Exception as e:
        new_db.execute("ROLLBACK;")
        print(f"Error during update: {e}")

if __name__ == '__main__':
    fix_partial_hashes()
