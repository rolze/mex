use crate::app::App;
use crate::domain::media::Status;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let mut items = Vec::new();

    let list_width = area.width.saturating_sub(2) as usize;
    let filename_width = list_width.saturating_sub(41);
    let list_height = area.height.saturating_sub(2).max(1) as usize;
    app.list_height = list_height;

    if app.cursor_pos < app.list_offset {
        app.list_offset = app.cursor_pos;
    } else if app.cursor_pos >= app.list_offset + list_height {
        app.list_offset = app.cursor_pos.saturating_sub(list_height - 1);
    }

    let start = app.list_offset;
    let end = (start + list_height).min(app.visible_rows.len());

    for i in start..end {
        let row = &app.visible_rows[i];
        match row {
            crate::app::ListRow::GroupSummary { key, start_idx, end_idx, level } => {
                let is_cursor = i == app.cursor_pos;
                let mut base_style = match level {
                    crate::app::ZoomLevel::Year => Style::default().fg(ratatui::style::Color::Yellow).add_modifier(Modifier::BOLD),
                    crate::app::ZoomLevel::Month => Style::default().fg(ratatui::style::Color::Cyan),
                    _ => Style::default().fg(ratatui::style::Color::Gray),
                };
                if is_cursor {
                    base_style = base_style.bg(app.theme.cursor_bg).add_modifier(Modifier::BOLD);
                }
                
                let mut num_images = 0;
                let mut num_videos = 0;
                let total_items = *end_idx - *start_idx;
                for j in *start_idx..*end_idx {
                    if let Some(media) = app.items.get(app.filtered_items[j]) {
                        match media.ext.to_lowercase().as_str() {
                            ".jpg" | ".jpeg" | ".png" | ".gif" | ".webp" => num_images += 1,
                            ".mp4" | ".mkv" | ".webm" | ".mov" | ".avi" | ".m4v" => num_videos += 1,
                            _ => {}
                        }
                    }
                }
                
                let text = generate_summary_text(*level, key, num_images, num_videos, total_items, filename_width);
                
                let spans = vec![
                    Span::styled(text, base_style),
                    Span::styled(
                        format!(" │ {}", "+"),
                        base_style.fg(app.theme.tag).add_modifier(Modifier::DIM)
                    )
                ];
                
                items.push(ListItem::new(Line::from(spans)).style(base_style));
            }
            crate::app::ListRow::Item(idx) => {
                let idx = *idx;
                if let Some(media) = app.items.get(idx) {
            let is_selected_for_batch = app.selected.contains(&idx);
            let is_cursor = i == app.cursor_pos;

            let mut base_style = Style::default();
            if media.status == Status::Trashed {
                base_style = base_style.add_modifier(Modifier::DIM);
            } else if media.missing_on_disk {
                base_style = base_style.bg(app.theme.missing_bg).fg(app.theme.missing_fg);
            } else if is_selected_for_batch {
                base_style = base_style.bg(app.theme.batch_bg);
            }

            if is_cursor {
                base_style = base_style
                    .bg(app.theme.cursor_bg)
                    .add_modifier(Modifier::BOLD);
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

            let folder = extract_folder_year(media.path_stem.as_ref());

            let prefix = format!("{}{} / ", folder, marker);
            let prefix_padded = format!("{:<8}", prefix); // Ensure alignment

            let filename = media.file_name().unwrap_or_else(|| String::from("unknown"));

            let mut spans = vec![Span::styled(prefix_padded, base_style)];

            let text_filter = app.filter.text.to_lowercase();
            let mut highlighted_spans = Vec::new();
            if text_filter.is_empty() {
                if !is_selected_for_batch {
                    if let Some(stem) = &media.path_stem {
                        let mut stem_spans =
                            crate::ui::semantic::colorize_stem(stem, base_style, &app.theme);
                        stem_spans.push(Span::styled(media.ext.clone(), base_style));
                        highlighted_spans.extend(stem_spans);
                    } else {
                        highlighted_spans.push(Span::styled(filename.clone(), base_style));
                    }
                } else {
                    highlighted_spans.push(Span::styled(filename.clone(), base_style));
                }
            } else {
                let parts: Vec<&str> = text_filter.split('*').filter(|s| !s.is_empty()).collect();
                let mut current_idx = 0;
                let fname_lower = filename.to_lowercase();

                for part in parts {
                    if let Some(pos) = fname_lower[current_idx..].find(part) {
                        let match_start = current_idx + pos;
                        let match_end = match_start + part.len();

                        if match_start > current_idx {
                            highlighted_spans.push(Span::styled(
                                filename[current_idx..match_start].to_string(),
                                base_style,
                            ));
                        }

                        highlighted_spans.push(Span::styled(
                            filename[match_start..match_end].to_string(),
                            base_style.bg(app.theme.filter_match_bg),
                        ));

                        current_idx = match_end;
                    }
                }
                if current_idx < filename.len() {
                    highlighted_spans.push(Span::styled(
                        filename[current_idx..].to_string(),
                        base_style,
                    ));
                }
            }

            let total_len: usize = highlighted_spans
                .iter()
                .map(|s| s.content.chars().count())
                .sum();
            if total_len > filename_width {
                let remove_count = total_len.saturating_sub(filename_width) + 1;
                let mut removed = 0;
                let mut new_spans = Vec::new();
                let mut prepended_ellipsis = false;

                for span in highlighted_spans {
                    let chars_count = span.content.chars().count();
                    if removed + chars_count <= remove_count {
                        removed += chars_count;
                        continue;
                    }

                    let mut s = span.content.clone();
                    if removed < remove_count {
                        let to_remove = remove_count - removed;
                        s = s.chars().skip(to_remove).collect();
                        removed += to_remove;
                    }

                    if !prepended_ellipsis {
                        new_spans.push(Span::styled("…", base_style));
                        prepended_ellipsis = true;
                    }
                    new_spans.push(Span::styled(s, span.style));
                }
                spans.extend(new_spans);
            } else {
                let padding = filename_width.saturating_sub(total_len);
                spans.extend(highlighted_spans);
                if padding > 0 {
                    spans.push(Span::styled(" ".repeat(padding), base_style));
                }
            }

            let tags = media.tags_packed.replace('\x1f', " ");
            let tags_truncated = if tags.is_empty() {
                format!("{:<30}", "—")
            } else if tags.chars().count() > 30 {
                let mut t: String = tags.chars().take(29).collect();
                t.push('…');
                t
            } else {
                format!("{:<30}", tags)
            };

            spans.push(Span::styled(
                format!(" │ {}", tags_truncated),
                base_style.fg(app.theme.tag).add_modifier(Modifier::DIM),
            ));

            items.push(ListItem::new(Line::from(spans)).style(base_style));
                }
            }
        }
    }

    let title = if app.filter.is_empty() {
        format!(
            " mex — {} / {} ",
            app.cursor_pos.saturating_add(1),
            app.items.len()
        )
    } else {
        format!(
            " mex — {} / {} / {} ",
            app.cursor_pos.saturating_add(1),
            app.visible_rows.len(),
            app.items.len()
        )
    };

    let title = if !app.selected.is_empty() {
        format!("{} ({} selected)", title, app.selected.len())
    } else {
        title
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(Span::styled(title, Style::default().fg(app.theme.title)));

    let list = List::new(items).block(block);

    let mut state = ListState::default();
    state.select(Some(app.cursor_pos.saturating_sub(app.list_offset)));

    f.render_stateful_widget(list, area, &mut state);
}

pub(crate) fn extract_folder_year(stem: Option<&String>) -> &str {
    if let Some(stem) = stem {
        stem.split('-').next().unwrap_or("????")
    } else {
        "????"
    }
}

pub(crate) fn generate_summary_text(level: crate::app::ZoomLevel, key: &str, num_images: usize, num_videos: usize, total_items: usize, filename_width: usize) -> String {
    let counts_str = if num_images > 0 || num_videos > 0 {
        let mut counts = Vec::new();
        if num_images > 0 { counts.push(format!("{} images", num_images)); }
        if num_videos > 0 { counts.push(format!("{} videos", num_videos)); }
        counts.join(", ")
    } else {
        format!("{} items", total_items)
    };
    
    let (prefix, text) = match level {
        crate::app::ZoomLevel::Year => {
            ("▶ ", format!("{} ({})", key, counts_str))
        }
        crate::app::ZoomLevel::Month => {
            ("  ▶ ", format!("{} ({})", key, counts_str))
        }
        _ => {
            ("    ▶ ", format!("{} ({})", key, counts_str))
        }
    };

    let summary_chars: String = text.chars().take(filename_width.saturating_sub(prefix.chars().count())).collect();
    let summary_padded = format!("{:<width$}", summary_chars, width = filename_width.saturating_sub(prefix.chars().count()));

    format!("{}{}", prefix, summary_padded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_folder_year() {
        assert_eq!(
            extract_folder_year(Some(&"2022-01-foo-bar".to_string())),
            "2022"
        );
        assert_eq!(extract_folder_year(Some(&"2023".to_string())), "2023");
        assert_eq!(extract_folder_year(None), "????");
    }

    #[test]
    fn test_generate_summary_text_fallback_items() {
        let result = generate_summary_text(crate::app::ZoomLevel::Month, "2000-09-21", 0, 0, 3, 40);
        let expected_prefix = "  ▶ ";
        let expected_summary = format!("{:<36}", "2000-09-21 (3 items)");
        assert_eq!(result, format!("{}{}", expected_prefix, expected_summary));
    }

    #[test]
    fn test_generate_summary_text_images_and_videos() {
        let result = generate_summary_text(crate::app::ZoomLevel::Slug, "2023-10-slug", 4, 2, 6, 50);
        let expected_prefix = "    ▶ ";
        let expected_summary = format!("{:<44}", "2023-10-slug (4 images, 2 videos)");
        assert_eq!(result, format!("{}{}", expected_prefix, expected_summary));
    }
}
