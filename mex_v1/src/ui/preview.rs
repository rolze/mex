use crate::app::App;
use crate::ui::theme;
use ratatui::{style::Modifier, 
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    if let Some(&idx) = app.filtered_items.get(app.cursor_pos) {
        if let Some(media) = app.items.get(idx) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // Metadata area at top
                    Constraint::Min(5),    // Image area at bottom
                ])
                .split(area);

            // Render Image in chunks[1]
            if app.picker.is_none() {
                use ratatui_image::picker::Picker;
                let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
                app.picker = Some(picker);
            }

            let path = &media.source_path;
            if !app.image_cache.contains_key(path) {
                // simple synchronous loading for prototype
                if let Ok(dyn_img) = image::open(path) {
                    if let Some(picker) = &mut app.picker {
                        let protocol = picker.new_resize_protocol(dyn_img);
                        app.image_cache.insert(path.clone(), protocol);
                    }
                }
            }

            if let Some(protocol) = app.image_cache.get_mut(path) {
                let img = ratatui_image::StatefulImage::default();
                f.render_stateful_widget(img, chunks[1], protocol);
            } else {
                let img_placeholder = Paragraph::new(" [ Image Loading failed ] ")
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(img_placeholder, chunks[1]);
            }

            // Render Metadata in chunks[0]
            let mut meta_text = vec![
                Line::from(vec![
                    Span::styled("Source: ", Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(
                        media.source_path.clone(),
                        Style::default().fg(theme::COLOR_TEXT),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Target: ", Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(
                        app.config
                            .target_root
                            .as_ref()
                            .and_then(|r| {
                                media
                                    .relative_path()
                                    .map(|p| r.join(p).to_string_lossy().into_owned())
                            })
                            .unwrap_or_else(|| "Unknown".to_string()),
                        Style::default().fg(theme::COLOR_TEXT),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Tags: ", Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(
                        media.tags_packed.replace('\x1f', " "),
                        Style::default().fg(theme::COLOR_SLUG),
                    ),
                ]),
            ];

            if let Some(orig) = &media.os_date {
                meta_text.push(Line::from(vec![
                    Span::styled("OS Date: ", Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(orig.clone(), Style::default().fg(theme::COLOR_TEXT)),
                ]));
            } else {
                meta_text.push(Line::from(vec![
                    Span::styled("Mex Date: ", Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(
                        media.mex_date.clone(),
                        Style::default().fg(theme::COLOR_TEXT),
                    ),
                ]));
            }

            let meta_p = Paragraph::new(meta_text)
                .block(Block::default().title(" Metadata ").borders(Borders::ALL));
            f.render_widget(meta_p, chunks[1]);
        } else {
            f.render_widget(
                Paragraph::new("No item").block(Block::default().borders(Borders::ALL)),
                area,
            );
        }
    } else {
        f.render_widget(
            Paragraph::new("No item").block(Block::default().borders(Borders::ALL)),
            area,
        );
    }
}
