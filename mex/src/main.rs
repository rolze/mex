mod app;
mod db;
mod ui;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, path::Path};

fn find_db() -> Option<String> {
    // Look for .mex.db relative to the crate directory or cwd
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

fn main() -> Result<()> {
    let db_path = find_db().context(
        "Could not find .mex.db. Run mex from the repository root or the mex/ sub-directory.",
    )?;

    let target_root = load_target_root(&db_path);

    let files = db::load_files(&db_path, "").context("Failed to load files from DB")?;

    let mut app = app::App::new(db_path, target_root, files);

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

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
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match (key.modifiers, key.code) {
                    // Quit
                    (_, KeyCode::Char('q')) => break,
                    (_, KeyCode::Esc) => {
                        if app.preview_open {
                            app.preview_open = false;
                            app.chafa_lines.clear();
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

                    // Preview toggle
                    (_, KeyCode::Enter) | (_, KeyCode::Char(' ')) => app.toggle_preview(),

                    // Filter: backspace
                    (_, KeyCode::Backspace) => app.pop_filter_char(),

                    // Filter: printable characters (skip navigation keys already handled)
                    (_, KeyCode::Char(c)) => app.push_filter_char(c),

                    _ => {}
                }
            }
        }
    }
    Ok(())
}
