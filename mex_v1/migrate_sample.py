import sqlite3
import shutil
import os
import pathlib

OLD_DB = '/srv/data/media/year/mex.db'
NEW_DB = '/srv/data/playground/year/mex_v1.db'
OLD_ROOT = '/srv/data/media/year'
NEW_ROOT = '/srv/data/playground/year'

os.makedirs(NEW_ROOT, exist_ok=True)

old_conn = sqlite3.connect(OLD_DB)
old_conn.row_factory = sqlite3.Row
new_conn = sqlite3.connect(NEW_DB)

# We want ~50 valid images/videos that actually have a target_path
rows = old_conn.execute("SELECT * FROM media WHERE target_path IS NOT NULL LIMIT 50").fetchall()

print(f"Found {len(rows)} rows to migrate.")

new_conn.execute("BEGIN TRANSACTION")

for r in rows:
    old_target_path = r['target_path']
    full_old_path = os.path.join(OLD_ROOT, old_target_path)
    full_new_path = os.path.join(NEW_ROOT, old_target_path)

    # Make dirs
    os.makedirs(os.path.dirname(full_new_path), exist_ok=True)
    
    # Copy file if exists
    if os.path.exists(full_old_path):
        shutil.copy2(full_old_path, full_new_path)
    else:
        print(f"Warning: file {full_old_path} not found on disk, skipping copy.")

    # Calculate path_stem
    # target_path is like "2024/2024-05-18-0001.jpg"
    basename = os.path.basename(old_target_path)
    ext = r['ext'] or os.path.splitext(basename)[1]
    path_stem = basename[:-len(ext)] if basename.endswith(ext) else basename

    status = r['status']
    if status == 'pending':
        status = 'imported'
    elif status == 'normal':
        status = 'normal'
    elif status not in ('imported', 'duplicate', 'trashed', 'deleted', 'normal'):
        status = 'imported'

    partial_hash = r['partial_hash']
    if not partial_hash:
        partial_hash = 'dummy_hash_' + r['id']

    # Insert into new DB
    try:
        new_conn.execute("""
            INSERT OR REPLACE INTO media 
            (id, source_path, path_stem, partial_hash, file_size, ext, derived_date, orig_exif_date, orig_xmp_date, orig_os_date, status, missing_on_disk, tags_packed, tag_types_packed, caption)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            r['id'],
            r['source_path'] or f"fake_source/{basename}",
            path_stem,
            partial_hash,
            r['file_size'] or 0,
            ext,
            r['derived_date'],
            r['exif_date'],
            r['xmp_date'],
            r['os_date'],
            status,
            0, # missing_on_disk
            "", # tags_packed
            "", # tag_types_packed
            r['caption_slug'] # Map caption_slug to caption for now
        ))
    except Exception as e:
        print(f"Error inserting {r['id']}: {e}")

new_conn.execute("COMMIT")
print("Migration completed.")
