mod app;
mod db;
mod ui;

use anyhow::{Context, Result};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::{
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
    errors::Errors,
};
use std::{
    io,
    path::Path,
    sync::mpsc::{self, Receiver},
    thread,
};

fn find_db() -> Option<String> {
    for candidate in &[".mex.db", "../.mex.db", "../../.mex.db"] {
        if Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn load_target_root(db_path: &str) -> String {
    if let Ok(conn) = rusqlite::Connection::open(db_path) {
        if let Ok(val) = conn.query_row(
            "SELECT value FROM config WHERE key = 'target_root'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            return val;
        }
    }
    String::new()
}

fn find_image_pool() -> Vec<std::path::PathBuf> {
    let candidates = ["mex-media-root", "../mex-media-root", "../../mex-media-root"];
    let image_exts = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];
    for dir in &candidates {
        let p = Path::new(dir);
        if p.is_dir() {
            if let Ok(entries) = std::fs::read_dir(p) {
                let mut pool: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| image_exts.contains(&e.to_lowercase().as_str()))
                            .unwrap_or(false)
                    })
                    .collect();
                pool.sort();
                if !pool.is_empty() {
                    return pool;
                }
            }
        }
    }
    vec![]
}

fn main() -> Result<()> {
    let db_path = find_db().context(
        "Could not find .mex.db. Run mex from the repository root or the mex/ sub-directory.",
    )?;

    let target_root = load_target_root(&db_path);
    let files = db::load_files(&db_path, "").context("Failed to load files from DB")?;
    let image_pool = find_image_pool();

    // Query terminal for graphics protocol/font-size (before entering alt screen).
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    // Channel: main -> worker (encode requests)
    let (tx_worker, rx_worker) = mpsc::channel::<ResizeRequest>();
    // Channel: worker -> main (encoded results)
    let (tx_result, rx_result) = mpsc::channel::<Result<ResizeResponse, Errors>>();

    // Background encoder thread — receives StatefulProtocol, resizes+encodes, sends back.
    thread::spawn(move || {
        while let Ok(request) = rx_worker.recv() {
            let _ = tx_result.send(request.resize_encode());
        }
    });

    let image_state = ThreadProtocol::new(tx_worker, None);

    let mut app = app::App::new(db_path, target_root, files, image_pool, picker, image_state);

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app, rx_result);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut app::App,
    rx_result: Receiver<Result<ResizeResponse, Errors>>,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Apply any completed image encodes without blocking.
        while let Ok(Ok(response)) = rx_result.try_recv() {
            app.image_state.update_resized_protocol(response);
        }

        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => match (key.modifiers, key.code) {
                    // Quit
                    (_, KeyCode::Char('q')) => break,
                    (_, KeyCode::Esc) => {
                        if app.preview_open {
                            app.preview_open = false;
                            app.image_state.empty_protocol();
                        } else {
                            app.clear_filter();
                        }
                    }

                    // Navigation
                    (_, KeyCode::Down) | (_, KeyCode::Char('j')) => app.move_down(),
                    (_, KeyCode::Up) | (_, KeyCode::Char('k')) => app.move_up(),
                    (_, KeyCode::Char('g')) => app.jump_top(),
                    (_, KeyCode::Char('G')) => app.jump_bottom(),
                    (KeyModifiers::CONTROL, KeyCode::Char('d')) => app.half_page_down(),
                    (KeyModifiers::CONTROL, KeyCode::Char('u')) => app.half_page_up(),
                    (_, KeyCode::PageDown) => app.page_down(),
                    (_, KeyCode::PageUp) => app.page_up(),

                    // Preview toggle
                    (_, KeyCode::Enter) | (_, KeyCode::Char(' ')) => app.toggle_preview(),

                    // Filter: backspace
                    (_, KeyCode::Backspace) => app.pop_filter_char(),

                    // Filter: printable characters
                    (_, KeyCode::Char(c)) => app.push_filter_char(c),

                    _ => {}
                },

                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => app.move_down(),
                    MouseEventKind::ScrollUp => app.move_up(),
                    MouseEventKind::Down(_) => {
                        app.select_at_row(mouse.row);
                    }
                    _ => {}
                },

                _ => {}
            }
        }
    }
    Ok(())
}
