use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const THUMB_SIZE: u32 = 256;

/// Returns the path to a 256 px JPEG thumbnail, generating it if needed.
pub fn ensure_thumbnail(source: &Path, cache_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(cache_dir)?;

    let meta = fs::metadata(source)?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let key = cache_key(source, size, mtime);
    let thumb = cache_dir.join(format!("{}.jpg", &key[..16]));

    if thumb.exists() {
        return Ok(thumb);
    }

    generate(source, &thumb)?;
    Ok(thumb)
}

fn cache_key(path: &Path, size: u64, mtime: u64) -> String {
    let mut h = Sha256::new();
    h.update(path.as_os_str().as_encoded_bytes());
    h.update(b"\0");
    h.update(size.to_le_bytes());
    h.update(mtime.to_le_bytes());
    hex::encode(h.finalize())
}

fn generate(source: &Path, dest: &Path) -> Result<()> {
    #[cfg(feature = "vips")]
    {
        use libvips::ops;
        let src = source
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 path: {}", source.display()))?;
        let dst = dest
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 path: {}", dest.display()))?;
        let opts = ops::ThumbnailOptions {
            height: THUMB_SIZE as i32,
            ..Default::default()
        };
        let image = ops::thumbnail_with_opts(src, THUMB_SIZE as i32, &opts)
            .map_err(|e| anyhow::anyhow!("libvips thumbnail {}: {e}", source.display()))?;
        image
            .image_write_to_file(dst)
            .map_err(|e| anyhow::anyhow!("libvips write {}: {e}", dest.display()))?;
        return Ok(());
    }

    #[cfg(not(feature = "vips"))]
    {
        let img = image::open(source)
            .map_err(|e| anyhow::anyhow!("cannot open {}: {e}", source.display()))?;
        let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE).into_rgb8();
        thumb
            .save_with_format(dest, image::ImageFormat::Jpeg)
            .map_err(|e| anyhow::anyhow!("cannot save thumbnail: {e}"))?;
        Ok(())
    }
}
