use crate::app::{App, DeslugifyState, EmptyTrashState, FixOsTimeState, ImportState, SlugifyState};
use crate::db::folder_of;
use crate::import::ImportStatus;
use crate::player::MpvStatus;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{thread::ThreadProtocol, StatefulImage};

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Import screens take over the full area (except filter bar).
    match &app.import.state {
        ImportState::Scanning {
            scanned,
            current_file,
        } => {
            let scanned = *scanned;
            let current_file = current_file.clone();
            draw_import_scanning(frame, app, area, scanned, &current_file);
            return;
        }
        ImportState::Preview { .. } => {
            draw_import_preview(frame, app, area);
            return;
        }
        ImportState::Copying {
            done,
            total,
            current_file,
            copied,
            skipped_dup,
            errors,
        } => {
            let done = *done;
            let total = *total;
            let copied = *copied;
            let skipped_dup = *skipped_dup;
            let errors = *errors;
            let current_file = current_file.clone();
            draw_import_copying(
                frame,
                app,
                area,
                done,
                total,
                &current_file,
                copied,
                skipped_dup,
                errors,
            );
            return;
        }
        _ => {}
    }

    // Deslugify progress overlay takes over the full screen while running.
    if let DeslugifyState::Running {
        done,
        total,
        current,
    } = &app.deslugify.state
    {
        let done = *done;
        let total = *total;
        let current = current.clone();
        draw_slug_progress(frame, app, area, done, total, &current, "Repairing slugs…");
        return;
    }

    // Slugify progress overlay takes over the full screen while running.
    if let SlugifyState::Running {
        done,
        total,
        current,
    } = &app.slugify.state
    {
        let done = *done;
        let total = *total;
        let current = current.clone();
        draw_slug_progress(frame, app, area, done, total, &current, "Slugifying files…");
        return;
    }

    // Fix-os-time progress overlay takes over the full screen while running.
    if let FixOsTimeState::Running {
        done,
        total,
        current,
    } = &app.fix_os_time.state
    {
        let done = *done;
        let total = *total;
        let current = current.clone();
        draw_slug_progress(frame, app, area, done, total, &current, "Fixing OS times…");
        return;
    }

    // Empty-trash preview / deleting overlay takes over the full screen.
    match &app.empty_trash.state {
        EmptyTrashState::Preview { .. } => {
            draw_empty_trash_preview(frame, app, area);
            return;
        }
        EmptyTrashState::Deleting { done, total } => {
            let done = *done;
            let total = *total;
            draw_empty_trash_deleting(frame, app, area, done, total);
            return;
        }
        _ => {}
    }

    // Version screen takes over the main area, leaving the filter bar visible.
    if app.version_screen {
        let outer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(area);
        draw_version_screen(frame, app, outer_chunks[0]);
        draw_filter(frame, app, outer_chunks[1]);
        return;
    }

    // Outer: bottom bar (3 lines) + main content
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let main_area = outer_chunks[0];
    let bottom_area = outer_chunks[1];

    // Bottom: filter/command bar on left, status box on right.
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(bottom_area);

    let filter_area = bottom_chunks[0];
    let status_area = bottom_chunks[1];

    // Main: left list + right preview (conditionally)
    let main_chunks = if app.image.preview_open {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(main_area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(main_area)
    };

    let list_area = main_chunks[0];
    app.list_height = list_area.height.saturating_sub(2) as usize;

    draw_list(frame, app, list_area);

    if app.image.preview_open {
        let preview_area = main_chunks[1];
        draw_preview(frame, app, preview_area);
    }

    // Import copy-progress is now a full-screen overlay (handled above in match).

    draw_filter(frame, app, filter_area);
    draw_status_box(frame, app, status_area);
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let total = app.all_files.len();
    let filtered = app.filtered.len();
    let pos = if filtered == 0 { 0 } else { app.selected + 1 };

    let title = if app.selection.is_empty() {
        if !app.is_filter_active() {
            format!(" mex — {} / {} ", pos, total)
        } else {
            format!(" mex — {} / {} / {} ", pos, filtered, total)
        }
    } else {
        let sel = app.selection.len();
        if !app.is_filter_active() {
            format!(" mex — {} / {} ({} selected) ", pos, total, sel)
        } else {
            format!(
                " mex — {} / {} / {} ({} selected) ",
                pos, filtered, total, sel
            )
        }
    };

    // Fixed column widths (folder is always a short year prefix)
    let inner_width = area.width.saturating_sub(2) as usize; // subtract borders
    const FOLDER_COL: usize = 6; // e.g. "2022/ "
    const TAGS_COL: usize = 30;
    const GAP: usize = 1;
    let filename_col = inner_width
        .saturating_sub(FOLDER_COL)
        .saturating_sub(TAGS_COL)
        .saturating_sub(GAP * 2)
        .max(10);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .map(|(i, f)| {
            let is_cursor = i == app.selected;
            let is_selected = app.selection.contains(&i);
            let is_trashed = f.status == "trashed";
            let is_missing = f.missing_on_disk && !is_trashed;

            let folder = folder_of(&f.target_path);
            // Reserve 1 char for the selection marker (between folder name and "/").
            let folder_cell = truncate_front(folder, FOLDER_COL - 2); // -2 for marker + /
            let folder_name = format!("{:<width$}", folder_cell, width = FOLDER_COL - 2);
            // Marker priority: trashed > missing > selected > normal.
            let show_dot = is_selected && !is_trashed && !is_missing;
            let marker_str = if is_trashed {
                "🗑"
            } else if is_missing {
                "!"
            } else if show_dot {
                "•"
            } else {
                " "
            };
            let marker_fg = if is_cursor {
                Color::Black
            } else if is_trashed {
                Color::DarkGray
            } else if is_missing {
                Color::Rgb(220, 80, 80)
            } else {
                Color::White
            };

            let base_style = if is_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_trashed {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            } else if is_missing {
                Style::default()
                    .fg(Color::Rgb(220, 100, 100))
                    .bg(Color::Rgb(60, 15, 15))
            } else if is_selected {
                Style::default().bg(Color::Rgb(50, 50, 90))
            } else {
                Style::default()
            };

            // ── Caption-edit mode: replace the filename area with an inline editor ──
            if is_cursor {
                if let Some(ref ed) = app.caption_edit {
                    // Derive structural components using the formal regex.
                    let re = crate::db::path_re();
                    let (stem_prefix, stem_suffix) = if let Some(caps) = re.captures(&f.target_path)
                    {
                        let year = caps.name("year").map(|m| m.as_str()).unwrap_or("0000");
                        let month = caps.name("month").map(|m| m.as_str()).unwrap_or("00");
                        let ext = caps.name("ext").map(|m| m.as_str()).unwrap_or("bin");

                        if let Some(day_m) = caps.name("day") {
                            let day = day_m.as_str();
                            let suffix = if caps.name("day_cap").is_some() {
                                // Pattern 3/4: suffix is collision counter (if exists) + extension
                                format!(
                                    "{}.{ext}",
                                    caps.name("day_coll")
                                        .map(|m| format!("-{}", m.as_str()))
                                        .unwrap_or_default()
                                )
                            } else {
                                // Pattern 5: suffix is counter + extension
                                format!(
                                    "{}.{ext}",
                                    caps.name("day_cnt")
                                        .map(|m| format!("-{}", m.as_str()))
                                        .unwrap_or_default()
                                )
                            };
                            (format!("{year}-{month}-{day}"), suffix)
                        } else if let Some(slug_m) = caps.name("slug") {
                            let slug = slug_m.as_str();
                            let cnt = caps.name("slug_cnt").map(|m| m.as_str()).unwrap_or("0001");
                            // Pattern 1/2: prefix includes slug and counter; suffix is extension
                            (format!("{year}-{month}-{slug}-{cnt}"), format!(".{ext}"))
                        } else {
                            ("unknown".to_string(), format!(".{ext}"))
                        }
                    } else {
                        // Fallback if somehow regex fails
                        let filename = f.target_path.rsplit('/').next().unwrap_or(&f.target_path);
                        let dot_pos = filename.rfind('.').unwrap_or(filename.len());
                        let (stem, ext) = (&filename[..dot_pos], &filename[dot_pos..]);
                        if !f.caption_slug.is_empty() {
                            let cap_suffix = format!("-{}", f.caption_slug);
                            (
                                stem.strip_suffix(cap_suffix.as_str())
                                    .unwrap_or(stem)
                                    .to_string(),
                                ext.to_string(),
                            )
                        } else {
                            (stem.to_string(), ext.to_string())
                        }
                    };

                    // Build caption field spans.
                    let pre_sel = ed.pre_selected && !ed.text.is_empty();
                    let cap_len = ed.text.chars().count();

                    let edit_style = Style::default()
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::UNDERLINED);
                    let dim_style = Style::default().fg(Color::DarkGray);

                    let mut filename_spans: Vec<Span> = Vec::new();
                    filename_spans.push(Span::styled(stem_prefix, base_style));

                    if pre_sel {
                        filename_spans.push(Span::styled("-", base_style));
                        // Entire pre-selected caption is underlined.
                        filename_spans.push(Span::styled(ed.text.clone(), edit_style));
                        filename_spans.push(Span::styled("_", edit_style));
                    } else if !ed.text.is_empty() {
                        filename_spans.push(Span::styled("-", base_style));
                        // Entire typed caption is underlined.
                        filename_spans.push(Span::styled(ed.text.clone(), edit_style));
                        filename_spans.push(Span::styled("_", edit_style));
                    } else {
                        filename_spans.push(Span::styled("_", edit_style));
                    }
                    filename_spans.push(Span::styled(stem_suffix, base_style));
                    filename_spans.push(Span::styled(format!(" [{cap_len}/42]"), dim_style));

                    let folder_cell = truncate_front(folder_of(&f.target_path), FOLDER_COL - 2);
                    let folder_name = format!("{:<width$}", folder_cell, width = FOLDER_COL - 2);
                    let folder_sep_fg = Color::Black;

                    let mut spans = vec![
                        Span::styled(folder_name, base_style.fg(folder_sep_fg)),
                        Span::styled(
                            marker_str,
                            base_style.fg(marker_fg).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("/", base_style.fg(folder_sep_fg)),
                    ];

                    // Truncate and pad the re-assembled filename line to match the list layout.
                    let assembled_line = Line::from(filename_spans);
                    let assembled_str = assembled_line.to_string();
                    let filename_cell = truncate_front(&assembled_str, filename_col);
                    let _filename_padded =
                        format!("{:<width$}", filename_cell, width = filename_col);

                    // We need to re-span it to keep styles, but since it's the cursor row we
                    // can just use the assembled_line for now as a simpler approximation,
                    // or better: just push the assembled spans.
                    // For now, let's keep it simple and just use the spans we built,
                    // but they might overflow.
                    spans.extend(assembled_line.spans);

                    let line = Line::from(spans);
                    return ListItem::new(line).style(base_style);
                }
            }

            let filename = f.target_path.rsplit('/').next().unwrap_or(&f.target_path);
            let filename_cell = truncate_front(filename, filename_col);
            // Pad to fixed width
            let filename_padded = format!("{:<width$}", filename_cell, width = filename_col);

            // Highlight slug (cyan+bold) and caption (dark-yellow+bold) independently.
            let slug_hi: &str = if !f.derived_slug.is_empty() {
                &f.derived_slug
            } else {
                ""
            };
            let caption_hi: &str = if !f.caption_slug.is_empty() {
                &f.caption_slug
            } else {
                ""
            };

            let base_fg = if is_cursor {
                Color::Black
            } else if is_trashed {
                Color::DarkGray
            } else if is_missing {
                Color::Rgb(220, 100, 100)
            } else {
                Color::White
            };
            let slug_fg = if is_cursor {
                Color::Black
            } else if is_trashed {
                Color::DarkGray
            } else if is_missing {
                Color::Rgb(220, 100, 100)
            } else {
                Color::Cyan
            };
            let caption_fg = if is_cursor {
                Color::Black
            } else if is_trashed {
                Color::DarkGray
            } else if is_missing {
                Color::Rgb(220, 100, 100)
            } else {
                Color::Yellow
            };

            // Build fg highlight regions: (start_byte, end_byte, fg_color); slug before caption.
            let mut regions: Vec<(usize, usize, Color)> = Vec::new();
            if !slug_hi.is_empty() {
                if let Some(pos) = filename_padded.find(slug_hi) {
                    regions.push((pos, pos + slug_hi.len(), slug_fg));
                }
            }
            if !caption_hi.is_empty() {
                let search_from = regions.last().map(|(_, e, _)| *e).unwrap_or(0);
                if let Some(rel) = filename_padded[search_from..].find(caption_hi) {
                    let pos = search_from + rel;
                    regions.push((pos, pos + caption_hi.len(), caption_fg));
                }
            }

            // Collect filter text match ranges for bg highlight (skip on cursor row).
            let filter_matches: Vec<(usize, usize)> = if !is_cursor && !app.filter.text.is_empty() {
                let needle = app.filter.text.to_lowercase();
                let haystack = filename_padded.to_lowercase();
                if needle.contains('*') {
                    // Wildcard: highlight the matched segments from the first valid match.
                    crate::app::filter_match_ranges(&haystack, &needle).unwrap_or_default()
                } else {
                    // Plain text: highlight every occurrence.
                    let mut matches = Vec::new();
                    let mut search_from = 0;
                    while let Some(rel) = haystack[search_from..].find(&needle) {
                        let start = search_from + rel;
                        let end = start + needle.len();
                        matches.push((start, end));
                        search_from = end;
                    }
                    matches
                }
            } else {
                Vec::new()
            };

            // Build spans by splitting at all region and filter-match boundaries.
            let filename_spans: Vec<Span> = {
                let mut pts: Vec<usize> = vec![0, filename_padded.len()];
                for (s, e, _) in &regions {
                    pts.push(*s);
                    pts.push(*e);
                }
                for (s, e) in &filter_matches {
                    pts.push(*s);
                    pts.push(*e);
                }
                pts.sort_unstable();
                pts.dedup();
                pts.windows(2)
                    .map(|w| {
                        let (seg_start, seg_end) = (w[0], w[1]);
                        let text = filename_padded[seg_start..seg_end].to_string();
                        let fg_region = regions
                            .iter()
                            .find(|(s, e, _)| *s <= seg_start && seg_end <= *e);
                        let in_filter = filter_matches
                            .iter()
                            .any(|(s, e)| *s <= seg_start && seg_end <= *e);
                        let fg = fg_region.map(|(_, _, c)| *c).unwrap_or(base_fg);
                        let mut style = base_style.fg(fg);
                        if fg_region.is_some() {
                            style = style.add_modifier(Modifier::BOLD);
                        }
                        if in_filter {
                            style = style.bg(Color::Rgb(90, 60, 0));
                        }
                        Span::styled(text, style)
                    })
                    .collect()
            };

            let tag_fg = if is_cursor {
                Color::Black
            } else if is_missing {
                Color::Rgb(180, 60, 60)
            } else {
                Color::Green
            };
            let sep_fg = if is_cursor {
                Color::Black
            } else {
                Color::DarkGray
            };
            let (tag_span_vec, tags_used) =
                tag_spans(&f.tags, base_style, tag_fg, sep_fg, TAGS_COL);
            let tags_padding = TAGS_COL.saturating_sub(tags_used);

            let folder_sep_fg = if is_cursor {
                Color::Black
            } else if is_missing {
                Color::Rgb(150, 50, 50)
            } else {
                Color::DarkGray
            };
            let mut spans = vec![
                Span::styled(folder_name, base_style.fg(folder_sep_fg)),
                Span::styled(
                    marker_str,
                    base_style.fg(marker_fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled("/", base_style.fg(folder_sep_fg)),
            ];
            spans.extend(filename_spans);
            spans.push(Span::raw(" "));
            spans.extend(tag_span_vec);
            if tags_padding > 0 {
                spans.push(Span::styled(
                    format!("{:width$}", "", width = tags_padding),
                    base_style,
                ));
            }

            let line = Line::from(spans);
            ListItem::new(line).style(base_style)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.selected.saturating_sub(app.scroll_offset)));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, area, &mut state);
}

/// Truncate `s` to `max_chars`, keeping the tail and prepending "…" if needed.
fn truncate_front(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let keep = max_chars.saturating_sub(1); // 1 char for "…"
        let start = chars.len() - keep;
        format!("…{}", chars[start..].iter().collect::<String>())
    }
}

/// Build tag spans that fit within `max_chars`, using ` │ ` as visual separator.
/// Returns the spans and the total number of characters used.
fn tag_spans(
    tags: &[String],
    base_style: Style,
    tag_fg: Color,
    sep_fg: Color,
    max_chars: usize,
) -> (Vec<Span<'static>>, usize) {
    if tags.is_empty() {
        let s = "—".to_string();
        let len = s.chars().count();
        return (vec![Span::styled(s, base_style.fg(tag_fg))], len);
    }

    const SEP: &str = " │ ";
    const SEP_LEN: usize = 3;

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;

    for (idx, tag) in tags.iter().enumerate() {
        if idx > 0 {
            if used + SEP_LEN > max_chars {
                break;
            }
            spans.push(Span::styled(SEP, base_style.fg(sep_fg)));
            used += SEP_LEN;
        }
        let remaining = max_chars.saturating_sub(used);
        if remaining == 0 {
            break;
        }
        let tag_str = truncate_end(tag, remaining);
        let tag_len = tag_str.chars().count();
        spans.push(Span::styled(tag_str, base_style.fg(tag_fg)));
        used += tag_len;
    }

    (spans, used)
}

/// Truncate `s` to `max_chars`, cutting the tail and appending "…" if needed.
fn truncate_end(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let keep = max_chars.saturating_sub(1);
        format!("{}…", chars[..keep].iter().collect::<String>())
    }
}

fn draw_preview(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Preview [{}] ", app.image.protocol_name));

    // Split: metadata at top (3 lines) + image below
    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    frame.render_widget(block, area);

    // Metadata — two columns: left (File, Date) | right (Tags, Slug, Caption)
    if let Some(file) = app.selected_file() {
        let meta_area = chunks[0];
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(meta_area);

        let date_str = if !file.os_date.is_empty() {
            file.os_date.as_str()
        } else if !file.derived_date.is_empty() {
            file.derived_date.as_str()
        } else {
            "—"
        };

        let orig_raw = if !file.orig_filename.is_empty() {
            file.orig_filename.as_str()
        } else {
            "—"
        };

        // Label "File  " / "Orig  " = 6 chars; truncate paths to fit the column.
        const LABEL_WIDTH: usize = 6;
        let avail = (cols[0].width as usize).saturating_sub(LABEL_WIDTH);
        let file_str = truncate_front(&file.target_path, avail);
        let orig_str = truncate_front(orig_raw, avail);

        let left = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("File  ", Style::default().fg(Color::DarkGray)),
                Span::raw(file_str),
            ]),
            Line::from(vec![
                Span::styled("Date  ", Style::default().fg(Color::DarkGray)),
                Span::styled(date_str, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Orig  ", Style::default().fg(Color::DarkGray)),
                Span::raw(orig_str),
            ]),
        ]);
        frame.render_widget(left, cols[0]);

        let slug_str = if !file.derived_slug.is_empty() {
            file.derived_slug.as_str()
        } else {
            "—"
        };
        let caption_str = if !file.caption_slug.is_empty() {
            file.caption_slug.as_str()
        } else {
            "—"
        };

        let mut tags_line = vec![Span::styled("Tags  ", Style::default().fg(Color::DarkGray))];
        if file.tags.is_empty() {
            tags_line.push(Span::styled("—", Style::default().fg(Color::Green)));
        } else {
            for (idx, tag) in file.tags.iter().enumerate() {
                if idx > 0 {
                    tags_line.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
                }
                tags_line.push(Span::styled(tag.clone(), Style::default().fg(Color::Green)));
            }
        }

        let right = Paragraph::new(vec![
            Line::from(tags_line),
            Line::from(vec![
                Span::styled("Slug  ", Style::default().fg(Color::DarkGray)),
                Span::styled(slug_str, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Capt  ", Style::default().fg(Color::DarkGray)),
                Span::styled(caption_str, Style::default().fg(Color::Yellow)),
            ]),
        ])
        .wrap(Wrap { trim: true });
        frame.render_widget(right, cols[1]);
    }

    // Image
    let image_area = chunks[1];
    if image_area.width > 2 && image_area.height > 2 {
        let img_widget = StatefulImage::<ThreadProtocol>::default();
        frame.render_stateful_widget(img_widget, image_area, &mut app.image.protocol);

        // Spinner overlay while encoding is in flight.
        if app.image.is_loading {
            let spin_char = SPINNER[app.image.spinner_frame % SPINNER.len()];
            let label = format!(" {} loading… ", spin_char);
            let label_width = label.chars().count() as u16;
            // Centre the spinner in the image area.
            let sx = image_area.x + image_area.width.saturating_sub(label_width) / 2;
            let sy = image_area.y + image_area.height / 2;
            if sx + label_width <= image_area.x + image_area.width
                && sy < image_area.y + image_area.height
            {
                let spinner_area = Rect::new(sx, sy, label_width, 1);
                let spinner_widget = Paragraph::new(Span::styled(
                    label,
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .alignment(Alignment::Center);
                frame.render_widget(spinner_widget, spinner_area);
            }
        }
    }
}

fn draw_filter(frame: &mut Frame, app: &App, area: Rect) {
    // Caption-edit mode takes over the filter bar with a hint line.
    if app.caption_edit.is_some() {
        let hint = Line::from(Span::styled(
            "F2: editing caption  —  ESC cancel  ·  ENTER confirm",
            Style::default().fg(Color::DarkGray),
        ));
        let widget =
            Paragraph::new(hint).block(Block::default().borders(Borders::ALL).title(" Caption "));
        frame.render_widget(widget, area);
        return;
    }

    let title = if app.cmd.input.is_some() {
        " Command "
    } else {
        " Filter "
    };

    let line = if let Some(ref cmd) = app.cmd.input {
        let mut spans: Vec<Span> = vec![Span::styled(
            format!(":{cmd}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )];

        // Show dim suffix for command-name autocomplete (only before first space).
        if !cmd.contains(' ') {
            if let Some(suggestion) = app.current_command_suggestion() {
                let typed_chars = cmd.chars().count();
                let sug_chars = suggestion.chars().count();
                if sug_chars > typed_chars {
                    let suffix: String = suggestion.chars().skip(typed_chars).collect();
                    spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
                }
            }
        } else if cmd.starts_with("import ") {
            // Dim suffix for import-path autocomplete.
            let typed_path = cmd.strip_prefix("import ").unwrap_or("");
            if let Some(suggestion) = app.current_import_path_suggestion() {
                let typed_chars = typed_path.chars().count();
                let sug_chars = suggestion.chars().count();
                if sug_chars > typed_chars {
                    let suffix: String = suggestion.chars().skip(typed_chars).collect();
                    spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
                }
            } else if typed_path.trim().is_empty() {
                spans.push(Span::styled("<path>", Style::default().fg(Color::DarkGray)));
            }
        } else if let Some(suggestion) = app.current_tag_arg_suggestion() {
            // Show dim suffix for tag-arg autocomplete.
            // typed_tail = the portion of the command that the suggestion completes.
            let typed_tail = if let Some(arg) = cmd.strip_prefix("tag ") {
                if let Some(at_pos) = arg.rfind('@') {
                    &arg[at_pos + 1..]
                } else {
                    arg
                }
            } else {
                // :untag — complete the last word
                cmd.rsplit(' ').next().unwrap_or("")
            };
            let typed_chars = typed_tail.chars().count();
            let sug_chars = suggestion.chars().count();
            if sug_chars > typed_chars {
                let suffix: String = suggestion.chars().skip(typed_chars).collect();
                spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
            }
        } else {
            // Show a dim param placeholder for commands that have required or optional
            // params but no dynamic suggestions (or where suggestions are absent).
            let dim = Style::default().fg(Color::DarkGray);
            if let Some(arg) = cmd.strip_prefix("create-view ") {
                if arg.trim().is_empty() {
                    spans.push(Span::styled("<name>", dim));
                }
            } else if let Some(arg) = cmd.strip_prefix("fix-date ") {
                if arg.trim().is_empty() {
                    spans.push(Span::styled("<yyyy-mm-dd>", dim));
                }
            } else if let Some(arg) = cmd.strip_prefix("tag ") {
                // Fallback placeholder when the library has no tags (otherwise the
                // autocomplete branch above already shows a dim suggestion suffix).
                if arg.trim().is_empty() {
                    spans.push(Span::styled("<name>", dim));
                }
            } else if let Some(arg) = cmd.strip_prefix("untag ") {
                // [name …] is optional; show it only when nothing has been typed yet.
                if arg.trim().is_empty() {
                    spans.push(Span::styled("[name …]", dim));
                }
            }
        }

        spans.push(Span::raw("_"));

        // Path-availability hint for `:import <path>` (shown after cursor).
        if cmd.starts_with("import ") {
            match &app.cmd.import_path_hint {
                Some(crate::app::ImportPathHint::Valid) => {
                    spans.push(Span::styled("  ✓", Style::default().fg(Color::Green)));
                }
                Some(crate::app::ImportPathHint::Missing) => {
                    spans.push(Span::styled("  ✗", Style::default().fg(Color::Red)));
                }
                Some(crate::app::ImportPathHint::NotADir) => {
                    spans.push(Span::styled(
                        "  ✗ not a dir",
                        Style::default().fg(Color::Red),
                    ));
                }
                None => {}
            }
        }

        Line::from(spans)
    } else if !app.is_filter_active() {
        let mut spans = vec![Span::styled(
            "Type to filter…  |  #tag  |  @type  |  Enter: preview  |  Ctrl+O: open  |  :: command  |  PgUp/PgDn: page",
            Style::default().fg(Color::DarkGray),
        )];
        if app.trashed_count > 0 {
            spans.push(Span::styled(
                format!("   [🗑 {} trashed]", app.trashed_count),
                Style::default().fg(Color::DarkGray),
            ));
        }
        Line::from(spans)
    } else {
        // Build a boolean expression: text AND (@types OR …) AND (#tags OR …)
        // Styling: AND/OR/() = DarkGray, /text = Bold (default fg), @type = Magenta+Bold, #tag = Cyan+Bold
        let dim = Style::default().fg(Color::DarkGray);
        let white = Style::default().add_modifier(Modifier::BOLD); // default fg = visible on any theme
        let cyan = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let mag = Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD);

        let mut spans: Vec<Span> = vec![];
        let mut need_and = false;

        let and_sep = || Span::styled(" AND ", dim);

        // ── text ──────────────────────────────────────────────────────────────
        // Show the / prefix whenever filter_mode is active (even with empty text)
        // so the user has a clear visual cue that filter editing is active.
        if app.filter.mode || !app.filter.text.is_empty() {
            spans.push(Span::styled(format!("/{}", app.filter.text), white));
            if !app.filter.text.is_empty() {
                need_and = true;
            }
        }

        // ── @type group ───────────────────────────────────────────────────────
        // Count: confirmed + maybe one being typed
        let typing_type = app.filter.tag_type_typing;
        let type_count = app.filter.tag_type_filters.len() + if typing_type { 1 } else { 0 };

        if type_count > 0 {
            if need_and {
                spans.push(and_sep());
            }
            let use_parens = type_count > 1;
            if use_parens {
                spans.push(Span::styled("(", dim));
            }

            for (i, ty) in app.filter.tag_type_filters.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" OR ", dim));
                }
                spans.push(Span::styled(format!("@{ty}"), mag));
            }

            if typing_type {
                if !app.filter.tag_type_filters.is_empty() {
                    spans.push(Span::styled(" OR ", dim));
                }
                spans.push(Span::styled(format!("@{}", app.filter.tag_type_input), mag));
                // dim autocomplete suffix
                if let Some(suggestion) = app.current_type_suggestion() {
                    let input_chars = app.filter.tag_type_input.chars().count();
                    let sug_chars = suggestion.chars().count();
                    if sug_chars > input_chars {
                        let suffix: String = suggestion.chars().skip(input_chars).collect();
                        spans.push(Span::styled(suffix, dim));
                    }
                }
            }

            if use_parens {
                spans.push(Span::styled(")", dim));
            }
            need_and = true;
        }

        // ── #tag group ────────────────────────────────────────────────────────
        let typing_tag = app.filter.tag_typing;
        let tag_count = app.filter.tag_filters.len() + if typing_tag { 1 } else { 0 };

        if tag_count > 0 {
            if need_and {
                spans.push(and_sep());
            }
            let use_parens = tag_count > 1;
            if use_parens {
                spans.push(Span::styled("(", dim));
            }

            for (i, tag) in app.filter.tag_filters.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" OR ", dim));
                }
                spans.push(Span::styled(format!("#{tag}"), cyan));
            }

            if typing_tag {
                if !app.filter.tag_filters.is_empty() {
                    spans.push(Span::styled(" OR ", dim));
                }
                spans.push(Span::styled(format!("#{}", app.filter.tag_input), cyan));
                // dim autocomplete suffix
                if let Some(suggestion) = app.current_suggestion() {
                    let input_chars = app.filter.tag_input.chars().count();
                    let sug_chars = suggestion.chars().count();
                    if sug_chars > input_chars {
                        let suffix: String = suggestion.chars().skip(input_chars).collect();
                        spans.push(Span::styled(suffix, dim));
                    }
                }
            }

            if use_parens {
                spans.push(Span::styled(")", dim));
            }
        }

        if app.filter.mode {
            spans.push(Span::raw("_"));
        }
        Line::from(spans)
    };

    let border_style = if app.filter.mode || app.cmd.input.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);
    let inner = block.inner(area);

    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(line), inner);
}

fn draw_status_box(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Status ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Transient status message takes priority over live mpv state.
    if let Some(ref msg) = app.cmd.status_message {
        let available = inner.width as usize;
        let truncated = truncate_end(msg, available);
        frame.render_widget(
            Paragraph::new(Span::styled(truncated, Style::default().fg(Color::Yellow))),
            inner,
        );
        return;
    }

    // Live mpv playback state.
    let (icon, text, color): (&str, &str, Color) = match &app.mpv.status {
        MpvStatus::Disconnected => ("", "—", Color::DarkGray),
        MpvStatus::Idle => ("⏹", "idle", Color::DarkGray),
        MpvStatus::Playing {
            filename,
            paused: false,
        } => ("▶", filename.as_str(), Color::Green),
        MpvStatus::Playing {
            filename,
            paused: true,
        } => ("⏸", filename.as_str(), Color::Yellow),
    };

    let line = if icon.is_empty() {
        Line::from(Span::styled(text, Style::default().fg(color)))
    } else {
        // Reserve 2 chars for "▶ " prefix; truncate filename to fit.
        let available = (inner.width as usize).saturating_sub(2);
        let truncated = truncate_end(text, available);
        Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::styled(truncated, Style::default().fg(color)),
        ])
    };

    frame.render_widget(Paragraph::new(line), inner);
}

// ── Import UI ─────────────────────────────────────────────────────────────────

/// Full-screen "Scanning…" overlay shown while the background thread walks the source.
fn draw_import_scanning(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    scanned: usize,
    current_file: &str,
) {
    let spinner = SPINNER[app.image.spinner_frame % SPINNER.len()];
    let file_line = if current_file.is_empty() {
        String::new()
    } else {
        format!("\n  {current_file}")
    };
    let text = format!("{spinner} Scanning… {scanned} files found{file_line}\n\n[Esc] abort");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Import — Scanning ")
        .style(Style::default().fg(Color::Cyan));
    let para = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

/// Full-screen dry-run preview: stats header + scrollable table of planned copies.
fn draw_import_preview(frame: &mut Frame, app: &mut App, area: Rect) {
    let (entries, scroll) = match &app.import.state {
        ImportState::Preview { entries, scroll } => (entries, *scroll),
        _ => return,
    };

    let total = entries.len();
    let pending = entries
        .iter()
        .filter(|e| e.status == ImportStatus::Pending)
        .count();
    let dups = entries
        .iter()
        .filter(|e| e.status == ImportStatus::Duplicate)
        .count();
    let skipped = entries
        .iter()
        .filter(|e| e.status == ImportStatus::Skipped)
        .count();
    let unknown = entries
        .iter()
        .filter(|e| e.status == ImportStatus::UnknownDate)
        .count();

    // Layout: stats bar (6 lines) + scrollable list + footer (3 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // stats + header row
            Constraint::Min(1),    // list
            Constraint::Length(3), // footer
        ])
        .split(area);

    // Stats block
    let stats_text = vec![
        Line::from(vec![
            Span::styled("  Total files found:", Style::default().fg(Color::White)),
            Span::styled(
                format!("{total:>5}"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Ready to copy:    ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{pending:>5}"),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Duplicate:        ", Style::default().fg(Color::White)),
            Span::styled(format!("{dups:>5}"), Style::default().fg(Color::DarkGray)),
            Span::styled("  (will be skipped)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Quality variant:  ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{skipped:>5}"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("  (will be skipped)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Unknown date:     ", Style::default().fg(Color::White)),
            Span::styled(format!("{unknown:>5}"), Style::default().fg(Color::Yellow)),
            Span::styled(
                "  (needs review — not copied)",
                Style::default().fg(Color::Yellow),
            ),
        ]),
    ];
    let stats_para = Paragraph::new(stats_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Import — Preview ")
            .style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(stats_para, chunks[0]);

    // Table of pending + unknown entries
    let list_height = chunks[1].height.saturating_sub(2) as usize;
    app.import.list_height = list_height;
    let visible_count = entries
        .iter()
        .filter(|e| e.status != ImportStatus::Skipped)
        .count();
    let visible: Vec<&crate::import::ImportEntry> = entries
        .iter()
        .filter(|e| e.status != ImportStatus::Skipped)
        .skip(scroll)
        .take(list_height)
        .collect();

    let width = chunks[1].width.saturating_sub(4) as usize;
    let src_col = (width / 2).min(50);
    let tgt_col = width.saturating_sub(src_col).saturating_sub(14);

    let items: Vec<ListItem> = visible
        .iter()
        .map(|e| {
            let src = e
                .source_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            let src = truncate_front(src, src_col);
            let tgt = e.target_path.as_deref().unwrap_or("— no date —");
            let tgt = truncate_end(tgt, tgt_col);
            let (status_char, status_color) = match e.status {
                ImportStatus::Pending => ('→', Color::Green),
                ImportStatus::UnknownDate => ('?', Color::Yellow),
                ImportStatus::Duplicate => ('=', Color::DarkGray),
                ImportStatus::Skipped => ('-', Color::DarkGray),
            };
            let date_src = &e.date_source;
            let slug_src = &e.slug_source;
            let (src_color, ext_warn) = if let Some(actual) = &e.wrong_ext {
                (Color::Red, format!(" !.{actual}"))
            } else {
                (Color::White, String::new())
            };
            let line = Line::from(vec![
                Span::styled(format!("{src:<src_col$}"), Style::default().fg(src_color)),
                Span::styled(
                    format!(" {status_char} "),
                    Style::default().fg(status_color),
                ),
                Span::styled(format!("{tgt:<tgt_col$}"), Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("  {date_src:>4}/{slug_src:<4}"),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(ext_warn, Style::default().fg(Color::Red)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let scroll_end = (scroll + list_height).min(visible_count);
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
        " {pending} ready  {scroll_end}/{visible_count}  ↑↓/PgDn/PgUp ",
    )));
    frame.render_widget(list, chunks[1]);

    // Footer
    let footer_line = if pending > 0 {
        Line::from(vec![
            Span::styled(
                "  y / Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — confirm import    ", Style::default().fg(Color::White)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — cancel", Style::default().fg(Color::White)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "  Esc",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — close  (nothing to import)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };
    let footer = Paragraph::new(footer_line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

/// Full-screen overlay shown while files are being copied.
#[allow(clippy::too_many_arguments)]
fn draw_import_copying(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    done: usize,
    total: usize,
    current_file: &str,
    copied: usize,
    skipped_dup: usize,
    errors: usize,
) {
    let spinner = SPINNER[app.image.spinner_frame % SPINNER.len()];
    let pct = (done * 100).checked_div(total).unwrap_or(0);

    // ASCII progress bar
    let bar_width = (area.width as usize).saturating_sub(8).min(50);
    let filled = (bar_width * done).checked_div(total).unwrap_or(0);
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

    let file_line = if current_file.is_empty() {
        String::new()
    } else {
        format!("\n  → {current_file}")
    };

    // Build stats line: only show non-zero counters to keep it clean.
    let mut stats_parts: Vec<String> = Vec::new();
    if copied > 0 {
        stats_parts.push(format!("✓ {copied} copied"));
    }
    if skipped_dup > 0 {
        stats_parts.push(format!("⊘ {skipped_dup} duplicate"));
    }
    if errors > 0 {
        stats_parts.push(format!("✗ {errors} error"));
    }
    let stats_line = if stats_parts.is_empty() {
        String::new()
    } else {
        format!("\n  {}", stats_parts.join("   "))
    };

    let text = format!(
        "{spinner}  {done} / {total}  ({pct}%)\n  {bar}{file_line}{stats_line}\n\n  [Esc] abort"
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Import — Copying ")
        .style(Style::default().fg(Color::Cyan));
    let para = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

/// Full-screen progress overlay used by deslugify and slugify while running in the background.
fn draw_slug_progress(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    done: usize,
    total: usize,
    current: &str,
    title: &str,
) {
    let spinner = SPINNER[app.image.spinner_frame % SPINNER.len()];
    let pct = (done * 100).checked_div(total).unwrap_or(0);

    let bar_width = (area.width as usize).saturating_sub(8).min(50);
    let filled = (bar_width * done).checked_div(total).unwrap_or(0);
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

    let file_line = if current.is_empty() {
        String::new()
    } else {
        format!("\n  → {current}")
    };

    let text = format!("{spinner}  {done} / {total}  ({pct}%)\n  {bar}{file_line}");

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .style(Style::default().fg(Color::Yellow));
    let para = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

/// Full-screen preview of trashed files to be permanently deleted by `:empty-trash`.
fn draw_empty_trash_preview(frame: &mut Frame, app: &mut App, area: Rect) {
    let (files, scroll) = match &app.empty_trash.state {
        EmptyTrashState::Preview { files, scroll } => (files.clone(), *scroll),
        _ => return,
    };

    let total = files.len();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header
            Constraint::Min(1),    // list
            Constraint::Length(3), // footer
        ])
        .split(area);

    // Header
    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            format!("  ⚠  {total} file(s) will be permanently deleted from disk."),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  Files are kept in DB as deleted (dedup guard). This cannot be undone.",
            Style::default().fg(Color::Yellow),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Empty Trash ")
            .style(Style::default().fg(Color::Red)),
    );
    frame.render_widget(header, chunks[0]);

    // File list
    let list_height = chunks[1].height.saturating_sub(2) as usize;
    app.import.list_height = list_height;

    let width = chunks[1].width.saturating_sub(4) as usize;
    let items: Vec<ListItem> = files
        .iter()
        .skip(scroll)
        .take(list_height)
        .map(|f| {
            let path = truncate_front(&f.target_path, width);
            ListItem::new(Line::from(Span::styled(
                format!("  🗑  {path}"),
                Style::default().fg(Color::DarkGray),
            )))
        })
        .collect();

    let scroll_end = (scroll + list_height).min(total);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {scroll_end}/{total}  ↑↓/PgDn/PgUp ")),
    );
    frame.render_widget(list, chunks[1]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "  y / Enter",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " — confirm permanent deletion    ",
            Style::default().fg(Color::White),
        ),
        Span::styled(
            "Esc",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" — cancel", Style::default().fg(Color::White)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

/// Full-screen overlay shown while `:empty-trash` is deleting files.
fn draw_empty_trash_deleting(frame: &mut Frame, app: &App, area: Rect, done: usize, total: usize) {
    let spinner = SPINNER[app.image.spinner_frame % SPINNER.len()];
    let pct = (done * 100).checked_div(total).unwrap_or(0);

    let bar_width = (area.width as usize).saturating_sub(8).min(50);
    let filled = (bar_width * done).checked_div(total).unwrap_or(0);
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

    let text = format!("{spinner}  {done} / {total}  ({pct}%)\n  {bar}");

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Empty Trash — Deleting… ")
        .style(Style::default().fg(Color::Red));
    let para = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

// ── :version screen ───────────────────────────────────────────────────────────

fn draw_version_screen(frame: &mut Frame, app: &App, area: Rect) {
    use ratatui::text::Text;

    let info = match &app.version_info {
        Some(i) => i,
        None => return,
    };

    let dim = Style::default().fg(Color::DarkGray);
    let label_style = Style::default().fg(Color::Cyan);
    let ok_style = Style::default().fg(Color::Green);
    let err_style = Style::default().fg(Color::Red);
    let val_style = Style::default();

    let mut lines: Vec<Line> = Vec::new();

    macro_rules! kv {
        ($key:expr, $val:expr) => {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<16}", $key), label_style),
                Span::styled($val.to_string(), val_style),
            ]));
        };
    }

    lines.push(Line::from(vec![
        Span::styled("  mex  ", label_style),
        Span::styled(
            info.mex_version.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("    OS: {} ({})", info.os, info.arch), dim),
    ]));

    let sem_ver_display = if info.sem_found {
        let vips = match info.sem_vips {
            Some(true) => "  vips: yes",
            Some(false) => "  vips: no",
            None => "",
        };
        format!("{}{}", info.sem_version, vips)
    } else {
        "not found".to_string()
    };
    lines.push(Line::from(vec![
        Span::styled("  sem  ", label_style),
        Span::styled(
            sem_ver_display,
            if info.sem_found {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                err_style
            },
        ),
        Span::styled(format!("    Config: {}", info.config_path), dim),
    ]));

    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "  Settings",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    kv!("target_root", &info.target_root);
    kv!("views_root", &info.views_root);

    let db_detail = if !info.db_file_size.is_empty() {
        format!(
            "{}  ({}, {} files)",
            info.db_path, info.db_file_size, info.total_files
        )
    } else {
        format!("{}  ({} files)", info.db_path, info.total_files)
    };
    kv!("db_path", db_detail);
    kv!("mpv_path", &info.mpv_path);
    kv!("image protocol", &info.image_protocol);

    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "  Dependencies",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for dep in &info.dep_statuses {
        let (icon, icon_style) = if dep.found {
            ("✓", ok_style)
        } else {
            ("✗", err_style)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<16}", dep.name), label_style),
            Span::styled(format!("{} ", icon), icon_style),
            Span::styled(dep.detail.clone(), if dep.found { val_style } else { dim }),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Press Esc to close", dim)));

    let text = Text::from(lines);
    let block = Block::default().borders(Borders::ALL).title(" Version ");
    let para = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}
