use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const THUMB_SIZE: i32 = 256;

/// Returns the path to a 256 px WebP thumbnail, generating it if needed.
/// Caller must ensure libvips is initialised (VipsApp alive) before calling.
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
    let thumb = cache_dir.join(format!("{}.webp", &key[..16]));

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
    let src = source
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF-8 source path"))?;
    let dst = dest
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF-8 dest path"))?;

    let opts = libvips::ops::ThumbnailOptions {
        height: THUMB_SIZE,
        size: libvips::ops::Size::Both,
        no_rotate: false,
        crop: libvips::ops::Interesting::None,
        linear: false,
        input_profile: None,
        output_profile: None,
        intent: libvips::ops::Intent::Relative,
        fail_on: libvips::ops::FailOn::None,
    };

    let thumb = libvips::ops::thumbnail_with_opts(src, THUMB_SIZE, &opts)
        .map_err(|e| anyhow::anyhow!("vips thumbnail: {e}"))?;

    libvips::ops::webpsave(&thumb, dst)
        .map_err(|e| anyhow::anyhow!("vips webpsave: {e}"))?;

    Ok(())
}
