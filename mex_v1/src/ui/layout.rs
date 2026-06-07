use crate::app::{App, Mode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders},
    Frame,
};

use super::{file_list, filter_bar, preview, status};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3), // Filter bar and Status box are bordered, so 3 lines
        ])
        .split(f.area());

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

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[1]);

    let (filter_border_style, status_border_style) = match app.mode {
        Mode::Filter | Mode::Command => (
            Style::default().fg(app.theme.border_active),
            Style::default().fg(app.theme.border),
        ),
        _ => (
            Style::default().fg(app.theme.border),
            Style::default().fg(app.theme.border),
        ),
    };

    let filter_title = match app.mode {
        Mode::Command | Mode::Caption => " Command ",
        _ => " Filter ",
    };

    let filter_block = Block::default()
        .title(ratatui::text::Span::styled(
            filter_title,
            Style::default().fg(app.theme.title),
        ))
        .borders(Borders::ALL)
        .border_style(filter_border_style);

    let status_block = Block::default()
        .title(ratatui::text::Span::styled(
            " Status ",
            Style::default().fg(app.theme.title),
        ))
        .borders(Borders::ALL)
        .border_style(status_border_style);

    let filter_inner = filter_block.inner(bottom_chunks[0]);
    let status_inner = status_block.inner(bottom_chunks[1]);

    f.render_widget(filter_block, bottom_chunks[0]);
    f.render_widget(status_block, bottom_chunks[1]);

    filter_bar::draw(f, app, filter_inner);

    if !matches!(app.mode, Mode::Command) {
        status::draw(f, app, status_inner);
    }
}
