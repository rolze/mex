/// Remote-controller abstraction for external media viewers/players.
///
/// Designed so that a future image viewer (with a different protocol) can
/// implement the same trait without changing any call-sites in app.rs.
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const MPV_SOCKET: &str = "/tmp/mex-mpv.sock";

/// Name of the Windows named pipe used in WSL mode (becomes `\\.\pipe\mex-mpv`).
const MPV_PIPE_NAME: &str = "mex-mpv";

// ── WSL / Windows helpers ─────────────────────────────────────────────────────

/// Returns `true` when mex is running inside WSL (any version).
pub fn is_wsl() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.to_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

/// Translate a Linux filesystem path to a Windows path string using `wslpath`.
///
/// Only called when `wsl_mode` is active (i.e. the mpv binary is a `.exe`).
/// Falls back to the original string representation on any error.
pub fn translate_path_for_player(path: &Path) -> String {
    let output = std::process::Command::new("wslpath")
        .arg("-w")
        .arg(path)
        .output();
    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_owned()
        }
        _ => path.to_string_lossy().into_owned(),
    }
}

/// Probe common locations for a Windows mpv binary accessible from WSL.
///
/// 1. Runs `cmd.exe /c where mpv` and converts the first result with `wslpath -u`.
/// 2. Checks a list of well-known installation paths under `/mnt/c/`.
///
/// Returns the first path that exists on the Linux filesystem, or `None`.
pub fn detect_windows_mpv() -> Option<String> {
    // 1. Try Windows PATH via cmd.exe
    if let Ok(out) = std::process::Command::new("cmd.exe")
        .args(["/c", "where mpv 2>nul"])
        .output()
    {
        if out.status.success() {
            let win_path = String::from_utf8_lossy(&out.stdout);
            let win_path = win_path.lines().next().unwrap_or("").trim();
            if !win_path.is_empty() {
                // Convert Windows path to WSL path
                if let Ok(conv) = std::process::Command::new("wslpath")
                    .arg("-u")
                    .arg(win_path)
                    .output()
                {
                    if conv.status.success() {
                        let wsl_path = String::from_utf8_lossy(&conv.stdout).trim().to_owned();
                        if std::path::Path::new(&wsl_path).exists() {
                            return Some(wsl_path);
                        }
                    }
                }
            }
        }
    }

    // 2. Check known common install locations
    let candidates = [
        "/mnt/c/Program Files/MPV Player/mpv.exe",
        "/mnt/c/Program Files/mpv/mpv.exe",
        "/mnt/c/Program Files (x86)/mpv/mpv.exe",
        "/mnt/c/tools/mpv/mpv.exe",
    ];
    for &path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_owned());
        }
    }

    None
}

/// File extensions recognised as video files (lower-cased).
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "m4v", "ts", "m2ts",
    "mpeg", "mpg", "ogv", "3gp", "divx", "rmvb",
];

// ── Status types ──────────────────────────────────────────────────────────────

/// Live playback state of the mpv process as seen by mex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MpvStatus {
    /// Socket not reachable — mpv is not running.
    Disconnected,
    /// mpv is running but idle (no file loaded).
    Idle,
    /// A file is loaded; `paused` reflects whether playback is paused.
    Playing { filename: String, paused: bool },
}

/// Internal messages produced by the background event-listener thread.
#[derive(Debug)]
pub enum MpvEvent {
    /// Socket connection was lost (or could not be established).
    Disconnected,
    /// `pause` property changed.
    Paused(bool),
    /// `filename` property changed (`None` when property was cleared).
    Filename(Option<String>),
    /// `idle-active` property changed.
    IdleActive(bool),
    /// `eof-reached` property changed — `true` when playback hit the end of the file.
    EofReached(bool),
}

// ── Event listener ────────────────────────────────────────────────────────────

/// Spawn a background thread that subscribes to mpv property events via the
/// IPC socket and forwards them to `tx` as `MpvEvent` messages.
///
/// The thread reconnects automatically whenever the socket disappears.
pub fn start_event_listener(socket_path: PathBuf, tx: mpsc::Sender<MpvEvent>) {
    std::thread::spawn(move || {
        loop {
            match UnixStream::connect(&socket_path) {
                Ok(stream) => {
                    // Clone for writing before moving stream into BufReader.
                    let mut write_stream = match stream.try_clone() {
                        Ok(s) => s,
                        Err(_) => {
                            let _ = tx.send(MpvEvent::Disconnected);
                            std::thread::sleep(Duration::from_millis(500));
                            continue;
                        }
                    };

                    // Short read timeout so the thread stays responsive.
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));

                    // Subscribe to the properties we care about.
                    let cmds = [
                        "{\"command\":[\"observe_property\",1,\"pause\"]}\n",
                        "{\"command\":[\"observe_property\",2,\"filename\"]}\n",
                        "{\"command\":[\"observe_property\",3,\"idle-active\"]}\n",
                        "{\"command\":[\"observe_property\",4,\"eof-reached\"]}\n",
                    ];
                    let mut ok = true;
                    for cmd in &cmds {
                        if write_stream.write_all(cmd.as_bytes()).is_err() {
                            ok = false;
                            break;
                        }
                    }
                    if !ok {
                        let _ = tx.send(MpvEvent::Disconnected);
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }

                    // Read property-change events until the socket closes.
                    let mut reader = BufReader::new(stream);
                    loop {
                        let mut line = String::new();
                        match reader.read_line(&mut line) {
                            Ok(0) => break, // EOF — mpv exited
                            Ok(_) => {
                                if let Some(event) = parse_mpv_event(&line) {
                                    if tx.send(event).is_err() {
                                        return; // main thread gone
                                    }
                                }
                            }
                            Err(e)
                                if e.kind() == std::io::ErrorKind::WouldBlock
                                    || e.kind() == std::io::ErrorKind::TimedOut =>
                            {
                                continue // read timeout — poll again
                            }
                            Err(_) => break, // real socket error
                        }
                    }

                    let _ = tx.send(MpvEvent::Disconnected);
                    std::thread::sleep(Duration::from_millis(500));
                }
                Err(_) => {
                    // mpv not running yet — retry later.
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
    });
}

/// Parse a single JSON line from the mpv IPC socket into an `MpvEvent`.
///
/// Returns `None` for lines that are not relevant property-change events
/// (e.g. command responses, other event types).
fn parse_mpv_event(line: &str) -> Option<MpvEvent> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if v.get("event")?.as_str()? != "property-change" {
        return None;
    }
    match v.get("name")?.as_str()? {
        "pause" => Some(MpvEvent::Paused(v.get("data")?.as_bool()?)),
        "filename" => {
            let filename = v.get("data").and_then(|d| d.as_str()).map(|s| s.to_owned());
            Some(MpvEvent::Filename(filename))
        }
        "idle-active" => Some(MpvEvent::IdleActive(v.get("data")?.as_bool()?)),
        "eof-reached" => Some(MpvEvent::EofReached(v.get("data")?.as_bool()?)),
        _ => None,
    }
}

// ── Trait ─────────────────────────────────────────────────────────────────────

pub trait RemoteController {
    /// Open a file for playback.
    fn open_file(&self, path: &Path) -> anyhow::Result<()>;
    /// Stop playback (keep the player process alive in idle mode).
    fn stop(&self) -> anyhow::Result<()>;
    /// Toggle play / pause.
    fn play_pause(&self) -> anyhow::Result<()>;
    /// Ensure playback is running (un-pause if paused).
    fn play(&self) -> anyhow::Result<()>;
    /// Return true if the player socket is reachable right now.
    fn is_connected(&self) -> bool;
}

// ── MpvController ─────────────────────────────────────────────────────────────

pub struct MpvController {
    socket_path: PathBuf,
    /// Path to the mpv binary (name on PATH or absolute path).
    mpv_bin: String,
    /// True when `mpv_bin` is a Windows executable (ends with `.exe`).
    /// In this mode a socat+npiperelay bridge is used and file paths are
    /// translated with `wslpath -w` before being sent to mpv.
    wsl_mode: bool,
}

impl MpvController {
    pub fn new(socket_path: impl Into<PathBuf>, mpv_bin: impl Into<String>) -> Self {
        let mpv_bin = mpv_bin.into();
        let wsl_mode = mpv_bin.to_lowercase().ends_with(".exe");
        Self { socket_path: socket_path.into(), mpv_bin, wsl_mode }
    }

    /// Ensure mpv is running and listening on the socket.
    ///
    /// Native Linux: spawn mpv with `--input-ipc-server=<socket>` if not already running.
    /// WSL mode: start the socat+npiperelay bridge first, then spawn Windows mpv.exe.
    fn ensure_running(&self) -> anyhow::Result<()> {
        if UnixStream::connect(&self.socket_path).is_ok() {
            return Ok(());
        }

        if self.wsl_mode {
            self.start_bridge_if_needed()?;
            // Spawn Windows mpv.exe; it connects to the named pipe that npiperelay bridges.
            std::process::Command::new(&self.mpv_bin)
                .arg("--idle=yes")
                .arg("--keep-open=yes")
                .arg(format!("--input-ipc-server={MPV_PIPE_NAME}"))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("view: could not spawn mpv: {e}"))?;
        } else {
            std::process::Command::new(&self.mpv_bin)
                .arg("--idle=yes")
                .arg("--keep-open=yes")
                .arg(format!("--input-ipc-server={}", self.socket_path.display()))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("view: could not spawn mpv: {e}"))?;
        }

        // Poll until socket appears (up to 2 s).
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(100));
            if UnixStream::connect(&self.socket_path).is_ok() {
                return Ok(());
            }
        }

        anyhow::bail!("view: mpv socket did not appear in time (is mpv installed?)")
    }

    /// Start the socat+npiperelay bridge that connects the Unix socket to
    /// the Windows named pipe `\\.\pipe\mex-mpv`.
    ///
    /// No-op if the socket is already reachable (bridge already running).
    fn start_bridge_if_needed(&self) -> anyhow::Result<()> {
        if UnixStream::connect(&self.socket_path).is_ok() {
            return Ok(());
        }

        // Remove any stale socket file before binding.
        let _ = std::fs::remove_file(&self.socket_path);

        let socket_str = self.socket_path.to_string_lossy();
        std::process::Command::new("socat")
            .arg(format!("UNIX-LISTEN:{socket_str},fork"))
            .arg(format!("EXEC:npiperelay.exe -ei -ep //./pipe/{MPV_PIPE_NAME},nofork"))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!(
                    "view: could not start socat bridge: {e} \
                     (WSL mode requires socat — run: apt install socat)"
                )
            })?;

        // Wait for socat to create the socket (up to 3 s).
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            if UnixStream::connect(&self.socket_path).is_ok() {
                return Ok(());
            }
        }

        anyhow::bail!(
            "view: socat bridge socket did not appear — \
             ensure npiperelay.exe is on your PATH (https://github.com/jstarks/npiperelay)"
        )
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
        let path_str = if self.wsl_mode {
            translate_path_for_player(path)
        } else {
            path.to_string_lossy().into_owned()
        };
        let escaped = path_str.replace('\\', "\\\\").replace('"', "\\\"");
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

    fn play(&self) -> anyhow::Result<()> {
        self.send_command(r#"{"command":["set_property","pause",false]}"#)
    }

    fn is_connected(&self) -> bool {
        UnixStream::connect(&self.socket_path).is_ok()
    }
}
