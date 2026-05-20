mod window;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sem", about = "Lightweight image viewer for mex")]
struct Args {
    /// Path to the image file to display
    path: PathBuf,

    /// Comma-separated tags to display under the image
    #[arg(long, default_value = "")]
    tags: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if !args.path.exists() {
        anyhow::bail!("sem: file not found: {}", args.path.display());
    }

    let tags: Vec<String> = if args.tags.is_empty() {
        vec![]
    } else {
        args.tags.split(',').map(|t| t.trim().to_owned()).collect()
    };

    window::run(args.path, tags)
}
