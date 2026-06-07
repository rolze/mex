use crate::app::{App, Mode};
use crate::ui::theme;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let mut spans = Vec::new();

    match app.mode {
        Mode::Normal => {
            if app.filter.is_empty() {
                spans.push(Span::styled(
                    " / to filter, : for command ",
                    Style::default().add_modifier(Modifier::DIM),
                ));
            } else {
                spans = build_filter_spans(app);
            }
        }
        Mode::Filter => {
            spans = build_filter_spans(app);

            // Add typing state
            if let Some(tag) = &app.tag_input {
                spans.push(Span::styled(" AND ", Style::default().add_modifier(Modifier::DIM)));
                spans.push(Span::styled(
                    format!("#{}", tag),
                    Style::default()
                        .fg(theme::COLOR_SLUG)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("_", Style::default().fg(theme::COLOR_TEXT)));
            } else if let Some(typ) = &app.type_input {
                spans.push(Span::styled(" AND ", Style::default().add_modifier(Modifier::DIM)));
                spans.push(Span::styled(
                    format!("@{}", typ),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("_", Style::default().fg(theme::COLOR_TEXT)));
            } else {
                spans.push(Span::styled("_", Style::default().fg(theme::COLOR_TEXT)));
            }
        }
        Mode::Command => {
            spans.push(Span::styled(
                format!(":{}", app.command_input),
                Style::default().fg(theme::COLOR_CAPTION),
            ));
            spans.push(Span::styled("_", Style::default().fg(theme::COLOR_TEXT)));
        }
        Mode::Caption => {
            // Dynamic filename assembly
            let mut folder = String::from("????");
            let mut slug = String::from("slug");
            let mut ext = String::from(".ext");
            let mut tags = String::new();
            
            if let Some(target) = app.selected.iter().next().or_else(|| app.filtered_items.get(app.cursor_pos)) {
                if let Some(media) = app.items.get(*target) {
                    ext = media.ext.clone();
                    if let Some(stem) = &media.path_stem {
                        let parts: Vec<&str> = stem.split('_').collect();
                        if !parts.is_empty() { folder = parts[0].to_string(); }
                        if parts.len() >= 2 { slug = parts[1].to_string(); }
                    }
                    tags = media.tags_packed.replace('\x1f', "_");
                }
            }

            spans.push(Span::styled(folder, Style::default().add_modifier(Modifier::DIM)));
            spans.push(Span::styled("_", Style::default().add_modifier(Modifier::DIM)));
            spans.push(Span::styled(slug, Style::default().fg(theme::COLOR_SLUG)));
            spans.push(Span::styled("_", Style::default().add_modifier(Modifier::DIM)));
            if !tags.is_empty() {
                spans.push(Span::styled(tags, Style::default().fg(Color::Green)));
                spans.push(Span::styled("_", Style::default().add_modifier(Modifier::DIM)));
            }
            spans.push(Span::styled(
                app.command_input.clone(),
                Style::default().fg(theme::COLOR_CAPTION),
            ));
            spans.push(Span::styled("_", Style::default().fg(theme::COLOR_TEXT))); // cursor
            spans.push(Span::styled(ext, Style::default().add_modifier(Modifier::DIM)));
        }
    }

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

fn build_filter_spans<'a>(app: &App) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut needs_and = false;

    if !app.filter.text.is_empty() {
        spans.push(Span::styled(
            format!("/{}", app.filter.text),
            Style::default()
                .fg(theme::COLOR_TEXT)
                .add_modifier(Modifier::BOLD),
        ));
        needs_and = true;
    }

    if !app.filter.types.is_empty() {
        if needs_and {
            spans.push(Span::styled(" AND ", Style::default().add_modifier(Modifier::DIM)));
        }
        if app.filter.types.len() > 1 {
            spans.push(Span::styled("(", Style::default().add_modifier(Modifier::DIM)));
        }
        for (i, typ) in app.filter.types.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" OR ", Style::default().add_modifier(Modifier::DIM)));
            }
            spans.push(Span::styled(
                format!("@{}", typ),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if app.filter.types.len() > 1 {
            spans.push(Span::styled(")", Style::default().add_modifier(Modifier::DIM)));
        }
        needs_and = true;
    }

    if !app.filter.tags.is_empty() {
        if needs_and {
            spans.push(Span::styled(" AND ", Style::default().add_modifier(Modifier::DIM)));
        }
        if app.filter.tags.len() > 1 {
            spans.push(Span::styled("(", Style::default().add_modifier(Modifier::DIM)));
        }
        for (i, tag) in app.filter.tags.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" OR ", Style::default().add_modifier(Modifier::DIM)));
            }
            spans.push(Span::styled(
                format!("#{}", tag),
                Style::default()
                    .fg(theme::COLOR_SLUG)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if app.filter.tags.len() > 1 {
            spans.push(Span::styled(")", Style::default().add_modifier(Modifier::DIM)));
        }
    }

    spans
}
