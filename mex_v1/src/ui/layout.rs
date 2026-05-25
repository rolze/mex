use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use crate::app::App;

use super::{file_list, filter_bar, preview, status};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0), // Main area
            Constraint::Length(1), // Filter bar / Status box
        ])
        .split(f.area());

    // Split bottom area into Filter bar (left) and Status box (right)
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ])
        .split(chunks[1]);

    if app.show_preview {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(chunks[0]);
        
        file_list::draw(f, app, main_chunks[0]);
        preview::draw(f, app, main_chunks[1]);
    } else {
        file_list::draw(f, app, chunks[0]);
    }

    filter_bar::draw(f, app, bottom_chunks[0]);
    status::draw(f, app, bottom_chunks[1]);
}
