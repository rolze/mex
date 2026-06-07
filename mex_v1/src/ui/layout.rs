use crate::app::{App, Mode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame,
};

use super::{file_list, filter_bar, preview, status};

pub fn draw(f: &mut Frame, app: &mut App) {
    let bottom_height = match app.mode {
        Mode::Filter | Mode::Command => 3,
        _ => 1,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),                // Main area
            Constraint::Length(bottom_height), // Filter bar / Status box
        ])
        .split(f.area());

    let (border, border_style) = match app.mode {
        Mode::Filter | Mode::Command => (Borders::ALL, Style::default().fg(Color::Yellow)),
        _ => (Borders::NONE, Style::default()),
    };

    let bottom_block = Block::default().borders(border).border_style(border_style);
    let bottom_inner = bottom_block.inner(chunks[1]);
    f.render_widget(bottom_block, chunks[1]);

    // Split bottom area into Filter bar (left) and Status box (right)
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(bottom_inner);

    if app.show_preview {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        file_list::draw(f, app, main_chunks[0]);
        preview::draw(f, app, main_chunks[1]);
    } else {
        file_list::draw(f, app, chunks[0]);
    }

    filter_bar::draw(f, app, bottom_chunks[0]);

    // Status messages are not shown in command mode.
    if !matches!(app.mode, Mode::Command) {
        status::draw(f, app, bottom_chunks[1]);
    }
}
