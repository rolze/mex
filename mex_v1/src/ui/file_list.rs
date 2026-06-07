use crate::app::App;
use crate::domain::media::Status;
use crate::ui::theme;
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
    let end = (start + list_height).min(app.filtered_items.len());

    for i in start..end {
        let idx = app.filtered_items[i];
        if let Some(media) = app.items.get(idx) {
            let is_selected_for_batch = app.selected.contains(&idx);
            let is_cursor = i == app.cursor_pos;

            let mut base_style = Style::default();
            if media.status == Status::Trashed {
                base_style = base_style.add_modifier(Modifier::DIM);
            } else if media.missing_on_disk {
                base_style = base_style
                    .bg(theme::COLOR_MISSING_BG)
                    .fg(theme::COLOR_MISSING_FG);
            } else if is_selected_for_batch {
                base_style = base_style.bg(theme::COLOR_BATCH_BG);
            }

            if is_cursor {
                base_style = Style::default()
                    .bg(theme::COLOR_CURSOR_BG)
                    .fg(theme::COLOR_CURSOR_FG)
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

            let folder = if let Some(stem) = &media.path_stem {
                stem.split('_').next().unwrap_or("????")
            } else {
                "????"
            };
            
            let prefix = format!("{}{} / ", folder, marker);
            let prefix_padded = format!("{:<8}", prefix); // Ensure alignment

            let filename = media.file_name().unwrap_or_else(|| String::from("unknown"));

            let mut spans = vec![Span::styled(
                prefix_padded,
                base_style,
            )];

            let text_filter = app.filter.text.to_lowercase();
            let mut highlighted_spans = Vec::new();
            if text_filter.is_empty() {
                if !is_selected_for_batch {
                    if let Some(stem) = &media.path_stem {
                        let mut stem_spans = crate::ui::semantic::colorize_stem(
                            stem,
                            base_style,
                        );
                        stem_spans.push(Span::styled(format!(".{}", media.ext), base_style));
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
                            base_style.bg(theme::COLOR_FILTER_MATCH_BG),
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
                base_style.add_modifier(Modifier::DIM),
            ));

            items.push(ListItem::new(Line::from(spans)).style(base_style));
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
            app.filtered_items.len(),
            app.items.len()
        )
    };

    let title = if !app.selected.is_empty() {
        format!("{} ({} selected)", title, app.selected.len())
    } else {
        title
    };

    let block = Block::default().borders(Borders::ALL).title(title);

    let list = List::new(items).block(block);

    let mut state = ListState::default();
    state.select(Some(app.cursor_pos.saturating_sub(app.list_offset)));

    f.render_stateful_widget(list, area, &mut state);
}
