use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    if let Some(&idx) = app.filtered_items.get(app.cursor_pos) {
        if let Some(media) = app.items.get(idx) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(5), // Image area
                    Constraint::Length(6), // Metadata area
                ])
                .split(area);

            // TODO: Render image in chunks[0] using ratatui-image
            let img_placeholder = Paragraph::new(" [ Image Preview placeholder ] ")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(img_placeholder, chunks[0]);

            // Render Metadata in chunks[1]
            let mut meta_text = vec![
                Line::from(vec![
                    Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(media.source_path.clone(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Tags: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(media.tags_packed.replace('\x1f', " "), Style::default().fg(Color::Cyan)),
                ]),
            ];

            if let Some(orig) = &media.orig_os_date {
                meta_text.push(Line::from(vec![
                    Span::styled("OS Date: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(orig.clone(), Style::default().fg(Color::White)),
                ]));
            } else {
                meta_text.push(Line::from(vec![
                    Span::styled("Derived Date: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(media.derived_date.clone(), Style::default().fg(Color::White)),
                ]));
            }

            let meta_p = Paragraph::new(meta_text)
                .block(Block::default().title(" Metadata ").borders(Borders::ALL));
            f.render_widget(meta_p, chunks[1]);

        } else {
            f.render_widget(Paragraph::new("No item").block(Block::default().borders(Borders::ALL)), area);
        }
    } else {
        f.render_widget(Paragraph::new("No item").block(Block::default().borders(Borders::ALL)), area);
    }
}
