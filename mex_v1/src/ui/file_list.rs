use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use crate::app::{App, Mode};
use crate::domain::media::Status;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let mut items = Vec::new();
    
    for (i, &idx) in app.filtered_items.iter().enumerate() {
        if let Some(media) = app.items.get(idx) {
            let is_selected_for_batch = app.selected.contains(&idx);
            let is_cursor = i == app.cursor_pos;
            
            let mut style = Style::default();
            if media.status == Status::Trashed {
                style = style.fg(Color::DarkGray);
            } else if media.missing_on_disk {
                style = style.bg(Color::Rgb(60, 15, 15)).fg(Color::Rgb(220, 100, 100));
            } else if is_selected_for_batch {
                style = style.bg(Color::Rgb(50, 50, 90));
            }

            if is_cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }

            let marker = if media.missing_on_disk {
                "!"
            } else if media.status == Status::Trashed {
                "🗑"
            } else if is_selected_for_batch {
                "•"
            } else {
                " "
            };

            // Basic formatting for now
            let folder = if let Some(stem) = &media.path_stem {
                if stem.len() >= 4 {
                    &stem[0..4]
                } else {
                    "????"
                }
            } else {
                "????"
            };
            
            let filename = media.file_name().unwrap_or_else(|| String::from("unknown"));
            
            let line = format!("{} {} / {:<30} {}", marker, folder, filename, media.tags_packed.replace('\x1f', " "));
            
            items.push(ListItem::new(line).style(style));
        }
    }

    let title = if app.filter.is_empty() {
        format!(" mex — {} / {} ", app.cursor_pos.saturating_add(1), app.items.len())
    } else {
        format!(" mex — {} / {} / {} ", app.cursor_pos.saturating_add(1), app.filtered_items.len(), app.items.len())
    };

    let title = if !app.selected.is_empty() {
        format!("{} ({} selected)", title, app.selected.len())
    } else {
        title
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title);
    
    let list = List::new(items).block(block);

    // ListState to handle scrolling
    let mut state = ListState::default();
    state.select(Some(app.cursor_pos));

    f.render_stateful_widget(list, area, &mut state);
}
