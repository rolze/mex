mod cache;
mod window;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[cfg(feature = "vips")]
const LONG_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\nvips: yes");
#[cfg(not(feature = "vips"))]
const LONG_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\nvips: no");

#[derive(Parser)]
#[command(name = "sem", version, long_version = LONG_VERSION, about = "Lightweight image viewer for mex")]
struct Args {
    /// Image path (single-image mode)
    path: Option<PathBuf>,

    /// Tags to display under the image (single-image mode)
    #[arg(long, default_value = "")]
    tags: String,

    /// Manifest file for grid mode (tab-separated: path TAB tags per line)
    #[arg(long, conflicts_with = "path")]
    files: Option<PathBuf>,

    /// Thumbnail cache directory (required with --files)
    #[arg(long)]
    cache_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    #[cfg(feature = "vips")]
    let _vips = libvips::VipsApp::new("sem", false)
        .map_err(|e| anyhow::anyhow!("cannot initialise libvips: {e}"))?;

    let args = Args::parse();

    match (args.path, args.files) {
        (Some(path), None) => {
            if !path.exists() {
                anyhow::bail!("sem: file not found: {}", path.display());
            }
            let tags = parse_tags(&args.tags);
            window::run_single(path, tags)
        }
        (None, Some(manifest)) => {
            let cache_dir = args
                .cache_dir
                .ok_or_else(|| anyhow::anyhow!("--cache-dir is required with --files"))?;
            let entries = parse_manifest(&manifest)?;
            if entries.is_empty() {
                anyhow::bail!("sem: manifest contains no image entries");
            }
            window::run_grid(entries, cache_dir)
        }
        (Some(_), Some(_)) => anyhow::bail!("sem: --files and a path cannot be combined"),
        (None, None) => anyhow::bail!("sem: provide an image path or --files <manifest>"),
    }
}

fn parse_tags(s: &str) -> Vec<String> {
    if s.is_empty() {
        vec![]
    } else {
        s.split(',').map(|t| t.trim().to_owned()).collect()
    }
}

fn parse_manifest(path: &PathBuf) -> Result<Vec<(PathBuf, Vec<String>)>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read manifest {}: {e}", path.display()))?;

    let entries = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let (img_path, tags_str) = line.split_once('\t').unwrap_or((line, ""));
            let img = PathBuf::from(img_path);
            let tags = parse_tags(tags_str);
            (img, tags)
        })
        .collect();

    Ok(entries)
}

