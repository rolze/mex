/// Remote-controller abstraction for external media viewers/players.
///
/// Designed so that a future image viewer (with a different protocol) can
/// implement the same trait without changing any call-sites in app.rs.
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const MPV_SOCKET: &str = "/tmp/mex-mpv.sock";

/// File extensions recognised as video files (lower-cased).
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "m4v", "ts", "m2ts",
    "mpeg", "mpg", "ogv", "3gp", "divx", "rmvb",
];

// ── Trait ─────────────────────────────────────────────────────────────────────

pub trait RemoteController {
    /// Open a file for playback.
    fn open_file(&self, path: &Path) -> anyhow::Result<()>;
    /// Stop playback (keep the player process alive in idle mode).
    fn stop(&self) -> anyhow::Result<()>;
    /// Toggle play / pause.
    fn play_pause(&self) -> anyhow::Result<()>;
    /// Return true if the player socket is reachable right now.
    fn is_connected(&self) -> bool;
}

// ── MpvController ─────────────────────────────────────────────────────────────

pub struct MpvController {
    socket_path: PathBuf,
}

impl MpvController {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self { socket_path: socket_path.into() }
    }

    /// Ensure mpv is running and listening on the socket.
    ///
    /// If the socket is not yet available, spawn a new mpv process with
    /// `--idle=yes` and poll for the socket to appear (up to 20 × 100 ms).
    fn ensure_running(&self) -> anyhow::Result<()> {
        if UnixStream::connect(&self.socket_path).is_ok() {
            return Ok(());
        }

        std::process::Command::new("mpv")
            .arg("--idle=yes")
            .arg("--keep-open=yes")
            .arg(format!("--input-ipc-server={}", self.socket_path.display()))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("view: could not spawn mpv: {e}"))?;

        // Poll until socket appears (up to 2 s).
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(100));
            if UnixStream::connect(&self.socket_path).is_ok() {
                return Ok(());
            }
        }

        anyhow::bail!("view: mpv socket did not appear in time (is mpv installed?)")
    }

    /// Write a single JSON command to the socket.
    fn send_command(&self, json: &str) -> anyhow::Result<()> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| anyhow::anyhow!("view: cannot connect to mpv socket: {e}"))?;
        stream
            .write_all(format!("{json}\n").as_bytes())
            .map_err(|e| anyhow::anyhow!("view: write to mpv socket failed: {e}"))?;
        Ok(())
    }
}

impl RemoteController for MpvController {
    fn open_file(&self, path: &Path) -> anyhow::Result<()> {
        self.ensure_running()?;
        let escaped = path.to_string_lossy().replace('"', "\\\"");
        self.send_command(&format!(
            r#"{{"command":["loadfile","{escaped}"]}}"#
        ))
    }

    fn stop(&self) -> anyhow::Result<()> {
        self.send_command(r#"{"command":["stop"]}"#)
    }

    fn play_pause(&self) -> anyhow::Result<()> {
        self.send_command(r#"{"command":["cycle","pause"]}"#)
    }

    fn is_connected(&self) -> bool {
        UnixStream::connect(&self.socket_path).is_ok()
    }
}
