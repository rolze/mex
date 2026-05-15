mod app;
mod config;
mod db;
mod import;
mod ui;

use anyhow::{Context, Result};
use crossterm::{
    event::{
        self, Event, KeyCode, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::{
    picker::{Picker, ProtocolType},
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

/// Resolve and validate target_root from local config. If missing or pointing
/// to a non-existent directory, prompt the user to enter a new path.
/// Returns the validated target_root or exits if the user cancels.
fn resolve_target_root() -> String {
    let mut cfg = config::load_config();

    loop {
        match config::validate_target_root(&cfg.target_root) {
            Ok(()) => {
                eprintln!("mex: media root: {}", cfg.target_root);
                return cfg.target_root;
            }
            Err(reason) => {
                let new_root = config::prompt_target_root(&cfg.target_root, &reason);
                match new_root {
                    Some(path) => {
                        cfg.target_root = path;
                        if let Err(e) = config::save_config(&cfg) {
                            eprintln!("mex: warning — could not save config: {e}");
                        }
                    }
                    None => {
                        eprintln!("mex: no media root configured; exiting.");
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let db_path = find_db().context(
        "Could not find .mex.db. Run mex from the repository root or the mex/ sub-directory.",
    )?;

    let target_root = resolve_target_root();
    let files = db::load_files(&db_path).context("Failed to load files from DB")?;

    // Query terminal for graphics protocol/font-size (before entering alt screen).
    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    // Allow manual override via MEX_PROTOCOL=kitty|sixel|iterm2|halfblocks.
    if let Ok(proto_env) = std::env::var("MEX_PROTOCOL") {
        let requested = match proto_env.to_lowercase().as_str() {
            "kitty"      => Some(ProtocolType::Kitty),
            "sixel"      => Some(ProtocolType::Sixel),
            "iterm2"     => Some(ProtocolType::Iterm2),
            "halfblocks" => Some(ProtocolType::Halfblocks),
            other => {
                eprintln!("mex: unknown MEX_PROTOCOL={other:?}; using auto-detected protocol");
                None
            }
        };
        if let Some(pt) = requested {
            picker.set_protocol_type(pt);
        }
    }

    let protocol_name = format!("{:?}", picker.protocol_type()).to_lowercase();

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

    let mut app = app::App::new(db_path, target_root, files, picker, image_state, protocol_name);

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app, rx_result);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
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

        if app.quit { break; }

        // Advance spinner animation regardless of events.
        app.tick();

        // Apply any completed image encodes without blocking.
        while let Ok(Ok(response)) = rx_result.try_recv() {
            app.on_encode_done(response);
        }

        // Poll import background thread (scan progress, copy progress, done).
        app.poll_import();

        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    // Handle import-preview / import-done screens first.
                    match &app.import_state {
                        app::ImportState::Preview { ref entries, .. } => {
                            let has_pending = entries.iter().any(|e| e.status == crate::import::ImportStatus::Pending);
                            match key.code {
                                KeyCode::Char('y') | KeyCode::Enter if has_pending => {
                                    app.confirm_import();
                                    continue;
                                }
                                KeyCode::Esc => {
                                    app.cancel_import();
                                    continue;
                                }
                                KeyCode::Down => {
                                    app.import_preview_scroll_down();
                                    continue;
                                }
                                KeyCode::Up => {
                                    app.import_preview_scroll_up();
                                    continue;
                                }
                                KeyCode::PageDown => {
                                    app.import_preview_page_down();
                                    continue;
                                }
                                KeyCode::PageUp => {
                                    app.import_preview_page_up();
                                    continue;
                                }
                                _ => continue, // swallow all other keys on preview
                            }
                        }
                        app::ImportState::Scanning { .. } | app::ImportState::Copying { .. } => {
                            // Esc cancels scan (copy cannot be undone)
                            if key.code == KeyCode::Esc {
                                if matches!(app.import_state, app::ImportState::Scanning { .. }) {
                                    app.cancel_import();
                                }
                                continue;
                            }
                            continue; // swallow other keys while busy
                        }
                        app::ImportState::Done(_) => {
                            app.import_state = app::ImportState::Idle;
                            // fall through so the keypress also registers normally
                        }
                        app::ImportState::Idle => {}
                    }
                    // Any keypress clears a displayed status message.
                    app.status_message = None;
                    match (key.modifiers, key.code) {
                    // Esc: cancel command → clear selection → close preview → clear filter
                    (_, KeyCode::Esc) => {
                        if app.command.is_some() {
                            app.cancel_command();
                        } else if !app.selection.is_empty() {
                            app.clear_selection();
                        } else if app.preview_open {
                            app.preview_open = false;
                        } else {
                            app.clear_filter();
                        }
                    }

                    // Navigation — arrow/page/ctrl keys only; no letter bindings
                    // Ctrl-modified arrows/home/end must come before their wildcard counterparts.
                    (KeyModifiers::SHIFT, KeyCode::Up)   => app.extend_selection_up(),
                    (KeyModifiers::SHIFT, KeyCode::Down) => app.extend_selection_down(),
                    (KeyModifiers::SHIFT, KeyCode::Home) => app.jump_slug_day_prev(),
                    (KeyModifiers::SHIFT, KeyCode::End)  => app.jump_slug_day_next(),
                    (_, KeyCode::Down)  => {
                        if app.tag_type_typing {
                            app.cycle_type_suggestion_down();
                        } else if app.tag_typing {
                            app.cycle_suggestion_down();
                        } else if app.command.is_some() {
                            app.cycle_command_suggestion_down();
                        } else {
                            app.move_down();
                        }
                    }
                    (_, KeyCode::Up)    => {
                        if app.tag_type_typing {
                            app.cycle_type_suggestion_up();
                        } else if app.tag_typing {
                            app.cycle_suggestion_up();
                        } else if app.command.is_some() {
                            app.cycle_command_suggestion_up();
                        } else {
                            app.move_up();
                        }
                    }
                    (_, KeyCode::Home)  => app.jump_home(),
                    (_, KeyCode::End)   => app.jump_end(),
                    (KeyModifiers::CONTROL, KeyCode::Char('d')) => app.half_page_down(),
                    (KeyModifiers::CONTROL, KeyCode::Char('u')) => app.half_page_up(),
                    (KeyModifiers::CONTROL, KeyCode::Char('a')) => app.select_all_or_none(),
                    (_, KeyCode::PageDown) => app.page_down(),
                    (_, KeyCode::PageUp)   => app.page_up(),

                    // Preview toggle / tag confirm / command execute
                    (_, KeyCode::Enter) => {
                        if app.tag_type_typing {
                            app.confirm_type_filter();
                        } else if app.tag_typing {
                            app.confirm_tag();
                        } else if app.command.is_some() {
                            app.execute_command();
                        } else {
                            app.toggle_preview();
                        }
                    }
                    (_, KeyCode::Char(' ')) if app.command.is_none() => app.toggle_selection(),

                    // Backspace: pop from command buffer or filter
                    (_, KeyCode::Backspace) => {
                        if app.command.is_some() {
                            app.pop_command_char();
                        } else {
                            app.pop_filter_char();
                        }
                    }

                    // Tab: complete current tag suggestion
                    (_, KeyCode::Tab) => app.tab_complete(),

                    // ':' enters command mode
                    (_, KeyCode::Char(':')) => app.enter_command_mode(),

                    // All other printable chars → command buffer or filter
                    (_, KeyCode::Char(c)) => {
                        if app.command.is_some() {
                            app.push_command_char(c);
                        } else {
                            app.push_filter_char(c);
                        }
                    }

                    _ => {}
                    }
                }

                _ => {}
            }
        }
    }
    Ok(())
}
