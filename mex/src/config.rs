use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;

/// Per-installation mex configuration.
/// Stored in `~/.config/mex/config.toml` (key = value, one per line).
/// Never written to `.mex.db` — the media DB is shared across devices.
#[derive(Default)]
pub struct Config {
    pub target_root: String,
    /// Root directory where `:create-view` materialises named view directories.
    pub views_root: String,
    /// Absolute (or relative) path to the `.mex.db` SQLite database.
    /// Resolved once at startup and persisted so subsequent launches don't
    /// need filesystem discovery.
    pub db_path: String,
    /// Path to the mpv binary.  Empty string means "not yet resolved" —
    /// `main.rs` will fill it in at startup.  Typical values:
    ///   - `"mpv"` on native Linux (name looked up via PATH)
    ///   - `"/mnt/c/Program Files/MPV Player/mpv.exe"` on WSL2
    pub mpv_path: String,
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("mex")
        .join("config.toml")
}

/// Read the local config file. Returns `Config::default()` if the file does
/// not exist yet (first-run scenario).
pub fn load_config() -> Config {
    let path = config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Config::default(),
    };
    let mut cfg = Config::default();
    for line in text.lines() {
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            if key == "target_root" {
                cfg.target_root = val.to_string();
            }
            if key == "views_root" {
                cfg.views_root = val.to_string();
            }
            if key == "db_path" {
                cfg.db_path = val.to_string();
            }
            if key == "mpv_path" {
                cfg.mpv_path = val.to_string();
            }
        }
    }
    cfg
}

/// Persist the config to `~/.config/mex/config.toml`, creating parent
/// directories as needed.
pub fn save_config(cfg: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create config dir {}", parent.display()))?;
    }
    let mut content = format!("target_root = {}\n", cfg.target_root);
    content.push_str(&format!("views_root = {}\n", cfg.views_root));
    content.push_str(&format!("db_path = {}\n", cfg.db_path));
    content.push_str(&format!("mpv_path = {}\n", cfg.mpv_path));
    std::fs::write(&path, content)
        .with_context(|| format!("could not write config file {}", path.display()))
}

/// Interactive prompt asking the user to enter (or confirm) the media root
/// path. Runs *before* the alternate screen / raw mode — plain terminal I/O.
///
/// Returns `Some(path)` on success, `None` if the user cancels (empty input
/// when no current value exists).
pub fn prompt_target_root(current: &str, reason: &str) -> Option<String> {
    eprintln!("mex: {reason}");
    if !current.is_empty() {
        eprintln!("  current: {current}");
        eprint!("  new path (Enter to keep current, Ctrl-C to quit): ");
    } else {
        eprint!("  media root path: ");
    }
    io::stderr().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim().to_string();

    if trimmed.is_empty() && !current.is_empty() {
        Some(current.to_string()) // keep existing
    } else if trimmed.is_empty() {
        None // nothing provided
    } else {
        Some(trimmed)
    }
}

/// Interactive prompt asking the user to enter (or confirm) the views root
/// path. Mirrors `prompt_target_root`.
///
/// Returns `Some(path)` on success, `None` if the user cancels.
pub fn prompt_views_root(current: &str, reason: &str) -> Option<String> {
    eprintln!("mex: {reason}");
    if !current.is_empty() {
        eprintln!("  current: {current}");
        eprint!("  new path (Enter to keep current, Ctrl-C to quit): ");
    } else {
        eprint!("  views root path: ");
    }
    io::stderr().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim().to_string();

    if trimmed.is_empty() && !current.is_empty() {
        Some(current.to_string())
    } else if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Check whether `target_root` is usable (non-empty, exists, is a directory,
/// is readable). Returns `Ok(())` on success or an error message string.
pub fn validate_target_root(root: &str) -> Result<(), String> {
    if root.is_empty() {
        return Err("media root is not configured".into());
    }
    let p = std::path::Path::new(root);
    if !p.exists() {
        return Err(format!("path does not exist: {root}"));
    }
    if !p.is_dir() {
        return Err(format!("path is not a directory: {root}"));
    }
    std::fs::read_dir(p).map_err(|e| format!("cannot read directory {root}: {e}"))?;
    Ok(())
}

/// Check whether `views_root` is configured (non-empty). The directory is
/// created at startup so existence is not checked here.
/// Returns `Ok(())` on success or an error message string.
pub fn validate_views_root(root: &str) -> Result<(), String> {
    if root.is_empty() {
        return Err("views root is not configured".into());
    }
    Ok(())
}

/// Interactive prompt asking the user to confirm or enter the path to the
/// `.mex.db` database file. Runs *before* the alternate screen / raw mode.
///
/// `current` — already-configured value (may be empty on first run).
/// `reason`  — explains why we are asking (e.g. "db not found").
///
/// When `current` is empty the default `./.mex.db` is offered.
/// Returns `Some(path)` on success, `None` if the user cancels.
pub fn prompt_db_path(current: &str, reason: &str) -> Option<String> {
    eprintln!("mex: {reason}");
    if !current.is_empty() {
        eprintln!("  current: {current}");
        eprint!("  new path (Enter to keep current, Ctrl-C to quit): ");
    } else {
        eprint!("  db path [default: ./.mex.db]: ");
    }
    io::stderr().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim().to_string();

    if trimmed.is_empty() && !current.is_empty() {
        Some(current.to_string())
    } else if trimmed.is_empty() {
        Some("./.mex.db".to_string()) // accept the default
    } else {
        Some(trimmed)
    }
}

/// Interactive prompt asking the user to confirm or enter the path to the mpv binary.
/// Runs *before* the alternate screen / raw mode.
///
/// `suggestion` — best guess at the correct path (shown as default).
/// Returns `Some(path)` on success, `None` if the user cancels.
pub fn prompt_mpv_path(suggestion: &str) -> Option<String> {
    eprintln!("mex: mpv binary not configured");
    if !suggestion.is_empty() {
        eprint!("  mpv path [default: {suggestion}]: ");
    } else {
        eprint!("  mpv path [default: mpv]: ");
    }
    io::stderr().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim().to_string();

    if trimmed.is_empty() {
        if !suggestion.is_empty() {
            Some(suggestion.to_string())
        } else {
            Some("mpv".to_string())
        }
    } else {
        Some(trimmed)
    }
}
