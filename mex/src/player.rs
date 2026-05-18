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
