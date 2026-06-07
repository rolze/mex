use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

pub enum MpvCommand {
    Play(String),
    TogglePause,
}

pub enum MpvEvent {
    Ended,
}

pub struct MpvContext {
    process: Option<Child>,
    pid: Option<u32>,
    cmd_tx: Sender<MpvCommand>,
    event_tx: Sender<MpvEvent>,
    event_rx: Receiver<MpvEvent>,
}

impl MpvContext {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<MpvCommand>();
        let (event_tx, event_rx) = mpsc::channel::<MpvEvent>();

        thread::spawn(move || {
            let get_sock_path = || -> String {
                format!("/tmp/mex-mpv-{}.sock", std::process::id())
            };
            
            for cmd in cmd_rx {
                match cmd {
                    MpvCommand::Play(path) => {
                        if let Ok(mut stream) = UnixStream::connect(get_sock_path()) {
                            let json = format!("{{ \"command\": [\"loadfile\", \"{}\"] }}\n", path);
                            let _ = stream.write_all(json.as_bytes());
                        }
                    }
                    MpvCommand::TogglePause => {
                        if let Ok(mut stream) = UnixStream::connect(get_sock_path()) {
                            let json = "{ \"command\": [\"cycle\", \"pause\"] }\n";
                            let _ = stream.write_all(json.as_bytes());
                        }
                    }
                }
            }
        });

        Self {
            process: None,
            pid: None,
            cmd_tx,
            event_tx,
            event_rx,
        }
    }

    pub fn play(&mut self, path: &str) {
        let sock_path = self.get_sock_path();
        
        let is_alive = if std::path::Path::new(&sock_path).exists() {
            UnixStream::connect(&sock_path).is_ok()
        } else {
            false
        };

        if is_alive {
            let _ = self.cmd_tx.send(MpvCommand::Play(path.to_string()));
        } else {
            // Clean up dead process if any
            if let Some(mut child) = self.process.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            
            let pid = std::process::id();
            self.pid = Some(pid);
            let sock_path = self.get_sock_path();
            let _ = std::fs::remove_file(&sock_path);

            if let Ok(child) = Command::new("mpv")
                .arg(format!("--input-ipc-server={}", sock_path))
                .arg("--force-window=yes")
                .arg(path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()
            {
                self.process = Some(child);
                
                // Spawn a listener thread
                let event_tx = self.event_tx.clone();
                let sock_path_clone = sock_path.clone();
                thread::spawn(move || {
                    // Wait a bit for the socket to be created
                    for _ in 0..50 {
                        if UnixStream::connect(&sock_path_clone).is_ok() {
                            break;
                        }
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                    
                    if let Ok(stream) = UnixStream::connect(&sock_path_clone) {
                        let reader = BufReader::new(stream);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                if l.contains("\"event\":\"end-file\"") || l.contains("\"event\": \"end-file\"") {
                                    let _ = event_tx.send(MpvEvent::Ended);
                                }
                            } else {
                                break;
                            }
                        }
                    }
                });
            }
        }
    }

    pub fn toggle_pause(&self) {
        let _ = self.cmd_tx.send(MpvCommand::TogglePause);
    }
    
    pub fn poll_events(&self) -> Vec<MpvEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    fn get_sock_path(&self) -> String {
        let pid = self.pid.unwrap_or_else(std::process::id);
        format!("/tmp/mex-mpv-{}.sock", pid)
    }
}

impl Drop for MpvContext {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
        let _ = std::fs::remove_file(self.get_sock_path());
    }
}
