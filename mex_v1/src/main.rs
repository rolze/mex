use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{io, time::Duration};

mod app;
mod config;
mod db;
mod domain;
mod services;
mod ui;

#[cfg(test)]
mod browse_tests;

fn main() -> Result<()> {
    // 1. Load configuration
    let config = config::Config::load()?;

    // Check if configuration requires prompting (UC-01 flow)
    // For now we assume they exist or use defaults, as per prototype, but we should handle DB creation.
    let mut db_path = config.db_path.clone();

    loop {
        if let Some(ref path) = db_path {
            if path.exists() {
                break;
            } else {
                println!("database file not found: {}", path.display());
            }
        }

        use std::io::Write;
        print!("new path: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            println!("path cannot be empty");
            continue;
        }

        db_path = Some(std::path::PathBuf::from(trimmed));
    }
    let db_path = db_path.unwrap();

    // 2. Initialize database
    let conn = db::init_db(&db_path)?;

    // 3. Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Create app state and run loop
    let mut app = app::App::new(config, conn)?;
    let res = run_app(&mut terminal, &mut app);

    // 5. Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut app::App) -> Result<()> {
    loop {
        terminal
            .draw(|f| ui::draw(f, app))
            .map_err(|e| anyhow::anyhow!("Draw error: {:?}", e))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                // Main loop key handling delegates to app
                if app.handle_key(key) {
                    return Ok(()); // Should exit
                }
            }
        }
        app.tick();
    }
}
