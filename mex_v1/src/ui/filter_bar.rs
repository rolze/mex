use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use crate::app::{App, Mode};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let mut spans = Vec::new();

    match app.mode {
        Mode::Normal => {
            if app.filter.is_empty() {
                spans.push(Span::styled(" / to filter, : for command ", Style::default().fg(Color::DarkGray)));
            } else {
                spans = build_filter_spans(app);
            }
        },
        Mode::Filter => {
            spans = build_filter_spans(app);
            
            // Add typing state
            if let Some(tag) = &app.tag_input {
                spans.push(Span::styled(" AND ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(format!("#{}", tag), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled("_", Style::default().fg(Color::White)));
            } else if let Some(typ) = &app.type_input {
                spans.push(Span::styled(" AND ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(format!("@{}", typ), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled("_", Style::default().fg(Color::White)));
            } else {
                spans.push(Span::styled("_", Style::default().fg(Color::White)));
            }
        },
        Mode::Command => {
            spans.push(Span::styled(format!(":{}", app.command_input), Style::default().fg(Color::Yellow)));
            spans.push(Span::styled("_", Style::default().fg(Color::White)));
        }
        Mode::Caption => {
            spans.push(Span::styled(format!("Caption: {}", app.command_input), Style::default().fg(Color::Yellow)));
            spans.push(Span::styled("_", Style::default().fg(Color::White)));
        }
    }

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

fn build_filter_spans<'a>(app: &App) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut needs_and = false;

    if !app.filter.text.is_empty() {
        spans.push(Span::styled(format!("/{}", app.filter.text), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
        needs_and = true;
    }

    if !app.filter.types.is_empty() {
        if needs_and {
            spans.push(Span::styled(" AND ", Style::default().fg(Color::DarkGray)));
        }
        if app.filter.types.len() > 1 {
            spans.push(Span::styled("(", Style::default().fg(Color::DarkGray)));
        }
        for (i, typ) in app.filter.types.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" OR ", Style::default().fg(Color::DarkGray)));
            }
            spans.push(Span::styled(format!("@{}", typ), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        }
        if app.filter.types.len() > 1 {
            spans.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
        }
        needs_and = true;
    }

    if !app.filter.tags.is_empty() {
        if needs_and {
            spans.push(Span::styled(" AND ", Style::default().fg(Color::DarkGray)));
        }
        if app.filter.tags.len() > 1 {
            spans.push(Span::styled("(", Style::default().fg(Color::DarkGray)));
        }
        for (i, tag) in app.filter.tags.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" OR ", Style::default().fg(Color::DarkGray)));
            }
            spans.push(Span::styled(format!("#{}", tag), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        }
        if app.filter.tags.len() > 1 {
            spans.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
        }
    }

    spans
}
