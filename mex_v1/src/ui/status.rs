use crate::app::App;
use ratatui::{style::Modifier, 
    layout::{Alignment, Rect},
    style::Style,
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let playback_icon = "▶"; // Simplified: always show play icon since we aren't tracking pause state currently.

    let text = if let Some(msg) = &app.status_message {
        format!("{} {}", playback_icon, msg)
    } else {
        format!("{} Ready", playback_icon)
    };

    let p = Paragraph::new(text)
        .style(Style::default().add_modifier(Modifier::DIM))
        .alignment(Alignment::Left);
    f.render_widget(p, area);
}
