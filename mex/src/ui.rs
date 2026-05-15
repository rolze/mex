use crate::app::{App, ImportState};
use crate::db::folder_of;
use crate::import::ImportStatus;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{StatefulImage, thread::ThreadProtocol};

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.size();

    // Import screens take over the full area (except filter bar).
    match &app.import_state {
        ImportState::Scanning { scanned, current_file } => {
            let scanned = *scanned;
            let current_file = current_file.clone();
            draw_import_scanning(frame, app, area, scanned, &current_file);
            return;
        }
        ImportState::Preview { .. } => {
            draw_import_preview(frame, app, area);
            return;
        }
        ImportState::Copying { done, total, current_file } => {
            let done = *done;
            let total = *total;
            let current_file = current_file.clone();
            draw_import_copying(frame, app, area, done, total, &current_file);
            return;
        }
        _ => {}
    }

    // Outer: filter bar at bottom (3 lines) + main content
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let main_area = outer_chunks[0];
    let filter_area = outer_chunks[1];

    // Main: left list + right preview (conditionally)
    let main_chunks = if app.preview_open {
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

    if app.preview_open {
        let preview_area = main_chunks[1];
        draw_preview(frame, app, preview_area);
    }

    // Import copy-progress is now a full-screen overlay (handled above in match).

    draw_filter(frame, app, filter_area);
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
            format!(" mex — {} / {} / {} ({} selected) ", pos, filtered, total, sel)
        }
    };

    // Fixed column widths (folder is always a short year prefix)
    let inner_width = area.width.saturating_sub(2) as usize; // subtract borders
    const FOLDER_COL: usize = 6;  // e.g. "2022/ "
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

            let folder = folder_of(&f.target_path);
            // Reserve 1 char for the selection marker (between folder name and "/").
            let folder_cell = truncate_front(folder, FOLDER_COL - 2); // -2 for marker + /
            let folder_name = format!("{:<width$}", folder_cell, width = FOLDER_COL - 2);
            // Dot only when the file is in the selection set (not just cursor position).
            let show_dot = is_selected;
            let marker_str = if show_dot { "•" } else { " " };
            let marker_fg = if is_cursor { Color::Black } else { Color::White };

            let base_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().bg(Color::Rgb(50, 50, 90))
            } else {
                Style::default()
            };

            let filename = f.target_path.rsplit('/').next().unwrap_or(&f.target_path);
            let filename_cell = truncate_front(filename, filename_col);
            // Pad to fixed width
            let filename_padded = format!("{:<width$}", filename_cell, width = filename_col);

            // Highlight slug (cyan+bold) and caption (dark-yellow+bold) independently.
            let slug_hi: &str    = if !f.derived_slug.is_empty()  { &f.derived_slug  } else { "" };
            let caption_hi: &str = if !f.caption_slug.is_empty() { &f.caption_slug } else { "" };

            let base_fg    = if is_cursor { Color::Black } else { Color::White };
            let slug_fg    = if is_cursor { Color::Black } else { Color::Cyan };
            let caption_fg = if is_cursor { Color::Black } else { Color::Yellow };

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
            let filter_matches: Vec<(usize, usize)> = if !is_cursor && !app.filter_text.is_empty() {
                let needle = app.filter_text.to_lowercase();
                let haystack = filename_padded.to_lowercase();
                let mut matches = Vec::new();
                let mut search_from = 0;
                while let Some(rel) = haystack[search_from..].find(&needle) {
                    let start = search_from + rel;
                    let end = start + needle.len();
                    matches.push((start, end));
                    search_from = end;
                }
                matches
            } else {
                Vec::new()
            };

            // Build spans by splitting at all region and filter-match boundaries.
            let filename_spans: Vec<Span> = {
                let mut pts: Vec<usize> = vec![0, filename_padded.len()];
                for (s, e, _) in &regions    { pts.push(*s); pts.push(*e); }
                for (s, e)    in &filter_matches { pts.push(*s); pts.push(*e); }
                pts.sort_unstable();
                pts.dedup();
                pts.windows(2).map(|w| {
                    let (seg_start, seg_end) = (w[0], w[1]);
                    let text = filename_padded[seg_start..seg_end].to_string();
                    let fg_region = regions.iter().find(|(s, e, _)| *s <= seg_start && seg_end <= *e);
                    let in_filter = filter_matches.iter().any(|(s, e)| *s <= seg_start && seg_end <= *e);
                    let fg = fg_region.map(|(_, _, c)| *c).unwrap_or(base_fg);
                    let mut style = base_style.fg(fg);
                    if fg_region.is_some() { style = style.add_modifier(Modifier::BOLD); }
                    if in_filter { style = style.bg(Color::Rgb(90, 60, 0)); }
                    Span::styled(text, style)
                }).collect()
            };

            let tag_fg = if is_cursor { Color::Black } else { Color::Green };
            let sep_fg = if is_cursor { Color::Black } else { Color::DarkGray };
            let (tag_span_vec, tags_used) = tag_spans(&f.tags, base_style, tag_fg, sep_fg, TAGS_COL);
            let tags_padding = TAGS_COL.saturating_sub(tags_used);

            let mut spans = vec![
                Span::styled(folder_name, base_style.fg(if is_cursor { Color::Black } else { Color::DarkGray })),
                Span::styled(marker_str, base_style.fg(marker_fg).add_modifier(Modifier::BOLD)),
                Span::styled("/", base_style.fg(if is_cursor { Color::Black } else { Color::DarkGray })),
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
        .title(format!(" Preview [{}] ", app.image_protocol_name));

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

        let date_str = if !file.os_date.is_empty() { file.os_date.as_str() }
                       else if !file.derived_date.is_empty() { file.derived_date.as_str() }
                       else { "—" };

        let orig_str = if !file.orig_filename.is_empty() { file.orig_filename.as_str() } else { "—" };

        let left = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("File  ", Style::default().fg(Color::DarkGray)),
                Span::raw(&file.target_path),
            ]),
            Line::from(vec![
                Span::styled("Date  ", Style::default().fg(Color::DarkGray)),
                Span::styled(date_str, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Orig  ", Style::default().fg(Color::DarkGray)),
                Span::raw(orig_str),
            ]),
        ])
        .wrap(Wrap { trim: true });
        frame.render_widget(left, cols[0]);

        let slug_str = if !file.derived_slug.is_empty() { file.derived_slug.as_str() }
                       else { "—" };
        let caption_str = if !file.caption_slug.is_empty() { file.caption_slug.as_str() }
                          else { "—" };

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
        frame.render_stateful_widget(img_widget, image_area, &mut app.image_state);

        // Spinner overlay while encoding is in flight.
        if app.is_loading {
            let spin_char = SPINNER[app.spinner_frame % SPINNER.len()];
            let label = format!(" {} loading… ", spin_char);
            let label_width = label.chars().count() as u16;
            // Centre the spinner in the image area.
            let sx = image_area.x + image_area.width.saturating_sub(label_width) / 2;
            let sy = image_area.y + image_area.height / 2;
            if sx + label_width <= image_area.x + image_area.width && sy < image_area.y + image_area.height {
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
    let title = if app.command.is_some() { " Command " } else { " Filter " };

    let line = if let Some(ref cmd) = app.command {
        let mut spans: Vec<Span> = vec![Span::styled(
            format!(":{cmd}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
        } else if let Some(arg) = cmd.strip_prefix("import ") {
            // Show dim "<path>" placeholder when no path has been typed yet.
            if arg.trim().is_empty() {
                spans.push(Span::styled("<path>", Style::default().fg(Color::DarkGray)));
            }
        }

        spans.push(Span::raw("_"));
        Line::from(spans)
    } else if !app.is_filter_active() {
        if let Some(ref msg) = app.status_message {
            Line::from(Span::styled(
                msg.clone(),
                Style::default().fg(Color::Yellow),
            ))
        } else {
            Line::from(Span::styled(
                "Type to filter…  |  #tag  |  @type  |  Enter: preview  |  :: command  |  PgUp/PgDn: page",
                Style::default().fg(Color::DarkGray),
            ))
        }
    } else {
        // Build a boolean expression: text AND (@types OR …) AND (#tags OR …)
        // Styling: AND/OR/() = DarkGray, /text = White+Bold, @type = Magenta+Bold, #tag = Cyan+Bold
        let dim   = Style::default().fg(Color::DarkGray);
        let white = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
        let cyan  = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
        let mag   = Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD);

        let mut spans: Vec<Span> = vec![];
        let mut need_and = false;

        let and_sep = || Span::styled(" AND ", dim);

        // ── text ──────────────────────────────────────────────────────────────
        if !app.filter_text.is_empty() {
            spans.push(Span::styled(format!("/{}", app.filter_text), white));
            need_and = true;
        }

        // ── @type group ───────────────────────────────────────────────────────
        // Count: confirmed + maybe one being typed
        let typing_type = app.tag_type_typing;
        let type_count = app.tag_type_filters.len() + if typing_type { 1 } else { 0 };

        if type_count > 0 {
            if need_and { spans.push(and_sep()); }
            let use_parens = type_count > 1;
            if use_parens { spans.push(Span::styled("(", dim)); }

            for (i, ty) in app.tag_type_filters.iter().enumerate() {
                if i > 0 { spans.push(Span::styled(" OR ", dim)); }
                spans.push(Span::styled(format!("@{ty}"), mag));
            }

            if typing_type {
                if !app.tag_type_filters.is_empty() { spans.push(Span::styled(" OR ", dim)); }
                spans.push(Span::styled(format!("@{}", app.tag_type_input), mag));
                // dim autocomplete suffix
                if let Some(suggestion) = app.current_type_suggestion() {
                    let input_chars = app.tag_type_input.chars().count();
                    let sug_chars = suggestion.chars().count();
                    if sug_chars > input_chars {
                        let suffix: String = suggestion.chars().skip(input_chars).collect();
                        spans.push(Span::styled(suffix, dim));
                    }
                }
            }

            if use_parens { spans.push(Span::styled(")", dim)); }
            need_and = true;
        }

        // ── #tag group ────────────────────────────────────────────────────────
        let typing_tag = app.tag_typing;
        let tag_count = app.tag_filters.len() + if typing_tag { 1 } else { 0 };

        if tag_count > 0 {
            if need_and { spans.push(and_sep()); }
            let use_parens = tag_count > 1;
            if use_parens { spans.push(Span::styled("(", dim)); }

            for (i, tag) in app.tag_filters.iter().enumerate() {
                if i > 0 { spans.push(Span::styled(" OR ", dim)); }
                spans.push(Span::styled(format!("#{tag}"), cyan));
            }

            if typing_tag {
                if !app.tag_filters.is_empty() { spans.push(Span::styled(" OR ", dim)); }
                spans.push(Span::styled(format!("#{}", app.tag_input), cyan));
                // dim autocomplete suffix
                if let Some(suggestion) = app.current_suggestion() {
                    let input_chars = app.tag_input.chars().count();
                    let sug_chars = suggestion.chars().count();
                    if sug_chars > input_chars {
                        let suffix: String = suggestion.chars().skip(input_chars).collect();
                        spans.push(Span::styled(suffix, dim));
                    }
                }
            }

            if use_parens { spans.push(Span::styled(")", dim)); }
        }

        spans.push(Span::raw("_"));
        Line::from(spans)
    };

    let para = Paragraph::new(line)
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(para, area);
}

// ── Import UI ─────────────────────────────────────────────────────────────────

/// Full-screen "Scanning…" overlay shown while the background thread walks the source.
fn draw_import_scanning(frame: &mut Frame, app: &App, area: Rect, scanned: usize, current_file: &str) {
    let spinner = SPINNER[app.spinner_frame % SPINNER.len()];
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
    let (entries, scroll) = match &app.import_state {
        ImportState::Preview { entries, scroll } => (entries, *scroll),
        _ => return,
    };

    let total = entries.len();
    let pending = entries.iter().filter(|e| e.status == ImportStatus::Pending).count();
    let dups = entries.iter().filter(|e| e.status == ImportStatus::Duplicate).count();
    let skipped = entries.iter().filter(|e| e.status == ImportStatus::Skipped).count();
    let unknown = entries.iter().filter(|e| e.status == ImportStatus::UnknownDate).count();

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
            Span::styled(format!("{total:>5}"), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Ready to copy:    ", Style::default().fg(Color::White)),
            Span::styled(format!("{pending:>5}"), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Duplicate:        ", Style::default().fg(Color::White)),
            Span::styled(format!("{dups:>5}"), Style::default().fg(Color::DarkGray)),
            Span::styled("  (will be skipped)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Quality variant:  ", Style::default().fg(Color::White)),
            Span::styled(format!("{skipped:>5}"), Style::default().fg(Color::DarkGray)),
            Span::styled("  (will be skipped)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Unknown date:     ", Style::default().fg(Color::White)),
            Span::styled(format!("{unknown:>5}"), Style::default().fg(Color::Yellow)),
            Span::styled("  (needs review — not copied)", Style::default().fg(Color::Yellow)),
        ]),
    ];
    let stats_para = Paragraph::new(stats_text)
        .block(Block::default().borders(Borders::ALL).title(" Import — Preview ").style(Style::default().fg(Color::Cyan)));
    frame.render_widget(stats_para, chunks[0]);

    // Table of pending + unknown entries
    let list_height = chunks[1].height.saturating_sub(2) as usize;
    app.import_list_height = list_height;
    let visible_count = entries.iter().filter(|e| e.status != ImportStatus::Skipped).count();
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
            let src = e.source_path.file_name()
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
                Span::styled(
                    format!("{src:<src_col$}"),
                    Style::default().fg(src_color),
                ),
                Span::styled(
                    format!(" {status_char} "),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("{tgt:<tgt_col$}"),
                    Style::default().fg(Color::Cyan),
                ),
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
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " {pending} ready  {scroll_end}/{visible_count}  ↑↓/PgDn/PgUp ",
        )));
    frame.render_widget(list, chunks[1]);

    // Footer
    let footer_line = if pending > 0 {
        Line::from(vec![
            Span::styled("  y / Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" — confirm import    ", Style::default().fg(Color::White)),
            Span::styled("Esc", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" — cancel", Style::default().fg(Color::White)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  Esc", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" — close  (nothing to import)", Style::default().fg(Color::DarkGray)),
        ])
    };
    let footer = Paragraph::new(footer_line)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

/// Full-screen overlay shown while files are being copied.
fn draw_import_copying(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    done: usize,
    total: usize,
    current_file: &str,
) {
    let spinner = SPINNER[app.spinner_frame % SPINNER.len()];
    let pct = if total > 0 { done * 100 / total } else { 0 };

    // ASCII progress bar
    let bar_width = (area.width as usize).saturating_sub(8).min(50);
    let filled = if total > 0 { bar_width * done / total } else { 0 };
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

    let file_line = if current_file.is_empty() {
        String::new()
    } else {
        format!("\n  ✓ {current_file}")
    };
    let text = format!(
        "{spinner}  {done} / {total} files  ({pct}%)\n  {bar}{file_line}"
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


