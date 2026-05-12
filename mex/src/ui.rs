use crate::app::App;
use crate::db::folder_of;
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
    app.list_area = list_area;

    draw_list(frame, app, list_area);

    if app.preview_open {
        let preview_area = main_chunks[1];
        draw_preview(frame, app, preview_area);
    }

    draw_filter(frame, app, filter_area);
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let total = app.all_files.len();
    let filtered = app.filtered.len();
    let pos = if filtered == 0 { 0 } else { app.selected + 1 };

    let title = if app.filter.is_empty() {
        format!(" mex — {} / {} ", pos, total)
    } else {
        format!(" mex — {} / {} / {} ", pos, filtered, total)
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
            let selected = i == app.selected;

            let folder = folder_of(&f.target_path);
            let folder_cell = truncate_front(folder, FOLDER_COL - 1); // -1 for "/"
            let folder_str = format!("{:<width$}/", folder_cell, width = FOLDER_COL - 1);

            let base_style = if selected {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let filename = f.target_path.rsplit('/').next().unwrap_or(&f.target_path);
            let filename_cell = truncate_front(filename, filename_col);
            // Pad to fixed width
            let filename_padded = format!("{:<width$}", filename_cell, width = filename_col);

            // Highlight the first non-empty of (derived_slug, caption_slug) inside the filename.
            let highlight: &str = if !f.derived_slug.is_empty() { &f.derived_slug }
                                  else if !f.caption_slug.is_empty() { &f.caption_slug }
                                  else { "" };

            let base_fg = if selected { Color::Black } else { Color::White };
            let filename_spans: Vec<Span> = if !highlight.is_empty() {
                if let Some(pos) = filename_padded.find(highlight) {
                    let (before, rest) = filename_padded.split_at(pos);
                    let (matched, after) = rest.split_at(highlight.len().min(rest.len()));
                    let hi_fg = if selected { Color::Black } else { Color::Cyan };
                    vec![
                        Span::styled(before.to_string(), base_style.fg(base_fg)),
                        Span::styled(matched.to_string(), base_style.fg(hi_fg).add_modifier(Modifier::BOLD)),
                        Span::styled(after.to_string(), base_style.fg(base_fg)),
                    ]
                } else {
                    vec![Span::styled(filename_padded, base_style.fg(base_fg))]
                }
            } else {
                vec![Span::styled(filename_padded, base_style.fg(base_fg))]
            };

            let tags_raw = if f.tags.is_empty() { "—".to_string() } else { f.tags.join(", ") };
            let tags_cell = truncate_end(&tags_raw, TAGS_COL);
            let tags_str = format!("{:<width$}", tags_cell, width = TAGS_COL);

            let mut spans = vec![
                Span::styled(folder_str, base_style.fg(if selected { Color::Black } else { Color::DarkGray })),
            ];
            spans.extend(filename_spans);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(tags_str, base_style.fg(if selected { Color::Black } else { Color::Green })));

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

    // Split: metadata at top (5 lines) + image below
    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(1)])
        .split(inner);

    frame.render_widget(block, area);

    // Metadata
    if let Some(file) = app.selected_file() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("File: ", Style::default().fg(Color::DarkGray)),
                Span::raw(&file.target_path),
            ]),
            Line::from(vec![
                Span::styled("Date: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&file.derived_date, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Ext:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(&file.ext),
            ]),
            Line::from(vec![
                Span::styled("Tags: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if file.tags.is_empty() { "—".to_string() } else { file.tags.join(", ") },
                    Style::default().fg(Color::Green),
                ),
            ]),
        ];
        if !file.derived_slug.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Slug: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&file.derived_slug, Style::default().fg(Color::Cyan)),
            ]));
        }
        if !file.caption_slug.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Capt: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&file.caption_slug, Style::default().fg(Color::Cyan)),
            ]));
        }
        let meta = Paragraph::new(lines).wrap(Wrap { trim: true });
        frame.render_widget(meta, chunks[0]);
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
    let (title, filter_text) = if let Some(ref cmd) = app.command {
        (
            " Command ",
            Span::styled(
                format!(":{cmd}_"),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        )
    } else if app.filter.is_empty() {
        (
            " Filter ",
            Span::styled(
                "Type to filter…  |  Enter: preview  |  :: command  |  PgUp/PgDn: page",
                Style::default().fg(Color::DarkGray),
            ),
        )
    } else {
        (
            " Filter ",
            Span::styled(
                format!("/{}_", app.filter),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        )
    };

    let para = Paragraph::new(Line::from(filter_text))
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(para, area);
}
