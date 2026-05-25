use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};
use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let text = if let Some(msg) = &app.status_message {
        msg.clone()
    } else {
        String::from("Status OK")
    };

    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Right);
    f.render_widget(p, area);
}
