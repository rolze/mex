use crate::app::App;
use crate::ui::theme;
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let text = if let Some(msg) = &app.status_message {
        msg.clone()
    } else {
        String::new()
    };

    let p = Paragraph::new(text)
        .style(Style::default().fg(theme::COLOR_DIM))
        .alignment(Alignment::Right);
    f.render_widget(p, area);
}
