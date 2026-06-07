use crate::app::App;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let playback_icon = "▶"; // Simplified

    let (text, style) = if let Some(msg) = &app.status_message {
        let is_error =
            msg.starts_with("Error") || msg.starts_with("DB error") || msg.starts_with("Failed");
        let is_success = msg.starts_with("Success")
            || msg.starts_with("copied:")
            || msg.starts_with("Caption applied");

        let msg_style = if is_error {
            Style::default()
                .fg(app.theme.error)
                .add_modifier(Modifier::BOLD)
        } else if is_success {
            Style::default()
                .fg(app.theme.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };

        (format!("{} {}", playback_icon, msg), msg_style)
    } else {
        (
            format!("{} Ready", playback_icon),
            Style::default().add_modifier(Modifier::DIM),
        )
    };

    let p = Paragraph::new(Span::styled(text, style)).alignment(Alignment::Left);
    f.render_widget(p, area);
}
