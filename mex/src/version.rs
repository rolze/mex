use std::path::Path;

/// Status of a runtime dependency (binary found on PATH or not).
pub struct DepStatus {
    pub name: String,
    pub found: bool,
    /// Full path when found, or an install hint when missing.
    pub detail: String,
}

/// All information shown by the `:version` screen.
pub struct VersionInfo {
    pub mex_version: String,
    /// Version string returned by `sem --version`, e.g. "0.2.0" or "0.2.0\nvips: yes".
    pub sem_version: String,
    pub sem_found: bool,
    /// Parsed vips flag from sem's version output, or None if sem not found.
    pub sem_vips: Option<bool>,
    pub os: String,
    pub arch: String,
    /// Tilde-abbreviated path to `~/.config/mex/config.toml`.
    pub config_path: String,
    pub target_root: String,
    pub views_root: String,
    pub db_path: String,
    pub mpv_path: String,
    pub image_protocol: String,
    /// Human-readable DB file size, e.g. "4.2 MB". Empty if stat failed.
    pub db_file_size: String,
    pub total_files: usize,
    pub dep_statuses: Vec<DepStatus>,
}

/// Collect version and environment info for the `:version` screen.
pub fn collect(
    db_path: &str,
    target_root: &str,
    views_root: &str,
    mpv_path: &str,
    image_protocol: &str,
    total_files: usize,
) -> VersionInfo {
    let mex_version = env!("CARGO_PKG_VERSION").to_string();

    // Query sem --version
    let (sem_found, sem_version, sem_vips) = query_sem_version();

    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();

    let home = std::env::var("HOME").unwrap_or_else(|_| String::new());
    let raw_config = format!("{}/.config/mex/config.toml", home);
    let config_path = abbreviate_home(&raw_config, &home);

    let db_file_size = stat_file_size(db_path);

    let dep_statuses = vec![
        probe_dep("sem", &["sem"]),
        probe_dep("mpv", &["mpv"]),
        probe_dep("socat", &["socat"]),
    ];

    VersionInfo {
        mex_version,
        sem_version,
        sem_found,
        sem_vips,
        os,
        arch,
        config_path,
        target_root: target_root.to_string(),
        views_root: views_root.to_string(),
        db_path: db_path.to_string(),
        mpv_path: mpv_path.to_string(),
        image_protocol: image_protocol.to_string(),
        db_file_size,
        total_files,
        dep_statuses,
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Run `sem --version` and parse the output.
/// Returns (found, version_string, vips_flag).
/// - found=false: binary not on PATH at all
/// - found=true, empty version: binary exists but errored
fn query_sem_version() -> (bool, String, Option<bool>) {
    let output = std::process::Command::new("sem").arg("--version").output();

    match output {
        Err(_) => (false, String::new(), None),
        Ok(out) => {
            // Clap writes version to stdout; runtime errors go to stderr.
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

            if !out.status.success() || stdout.trim().is_empty() {
                // Binary found but failed to run — show first line of error.
                let err_line = stderr
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("unknown error")
                    .trim()
                    .to_string();
                return (true, format!("ERROR: {err_line}"), None);
            }

            let mut version_line = String::new();
            let mut vips: Option<bool> = None;
            for line in stdout.lines() {
                let l = line.trim();
                if version_line.is_empty() && !l.is_empty() {
                    // clap outputs "sem X.Y.Z" — strip the binary name prefix.
                    version_line = if let Some(v) = l.strip_prefix("sem ") {
                        v.to_string()
                    } else {
                        l.to_string()
                    };
                }
                if l.starts_with("vips:") {
                    vips = Some(l.contains("yes"));
                }
            }
            if version_line.is_empty() {
                version_line = "unknown".to_string();
            }
            (true, version_line, vips)
        }
    }
}

/// Return the size of a file as a human-readable string.
fn stat_file_size(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    match std::fs::metadata(Path::new(path)) {
        Ok(m) => human_bytes(m.len()),
        Err(_) => String::new(),
    }
}

fn human_bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if n >= GB {
        format!("{:.1} GB", n as f64 / GB as f64)
    } else if n >= MB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{:.1} KB", n as f64 / KB as f64)
    } else {
        format!("{} B", n)
    }
}

/// Check if any of `names` resolves to an executable on PATH.
fn probe_dep(label: &str, names: &[&str]) -> DepStatus {
    for name in names {
        if let Some(path) = which(name) {
            return DepStatus {
                name: label.to_string(),
                found: true,
                detail: path,
            };
        }
    }
    let hint = install_hint(label);
    DepStatus {
        name: label.to_string(),
        found: false,
        detail: hint,
    }
}

/// Resolve a binary name via PATH. Returns the full path string on success.
fn which(name: &str) -> Option<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in path_var.split(':') {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn install_hint(name: &str) -> String {
    match name {
        "sem" => "not found — install from: https://github.com/rolze/mex".to_string(),
        "mpv" => "not found — install with: sudo apt install mpv".to_string(),
        "socat" => {
            "not found — required for mpv IPC; install with: sudo apt install socat".to_string()
        }
        _ => format!("not found — install {name}"),
    }
}

/// Replace the home directory prefix with `~`.
fn abbreviate_home(path: &str, home: &str) -> String {
    if !home.is_empty() && path.starts_with(home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}
