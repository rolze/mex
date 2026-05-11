use crate::app::App;
use crate::db::folder_of;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

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
    app.list_height = list_area.height.saturating_sub(2) as usize; // subtract border
    app.list_area = list_area;

    draw_list(frame, app, list_area);

    if app.preview_open {
        draw_preview(frame, app, main_chunks[1]);
    }

    draw_filter(frame, app, filter_area);
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let count = app.filtered.len();
    let title = if app.filter.is_empty() {
        format!(" mex — {} files ", count)
    } else {
        format!(" mex — {} / {} ", count, app.all_files.len())
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

            let filename = f.target_path.rsplit('/').next().unwrap_or(&f.target_path);
            let filename_cell = truncate_front(filename, filename_col);
            let filename_str = format!("{:<width$}", filename_cell, width = filename_col);

            let tags_raw = if f.tags.is_empty() {
                "—".to_string()
            } else {
                f.tags.join(", ")
            };
            let tags_cell = truncate_end(&tags_raw, TAGS_COL);
            let tags_str = format!("{:<width$}", tags_cell, width = TAGS_COL);

            let base_style = if selected {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(folder_str, base_style.fg(if selected { Color::Black } else { Color::DarkGray })),
                Span::styled(filename_str, base_style.fg(if selected { Color::Black } else { Color::White })),
                Span::raw(" "),
                Span::styled(tags_str, base_style.fg(if selected { Color::Black } else { Color::Green })),
            ]);
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

fn draw_preview(frame: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(file) = app.selected_file() {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("File:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(&file.target_path),
            ]),
            Line::from(vec![
                Span::styled("Date:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(&file.derived_date, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Ext:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(&file.ext),
            ]),
            Line::from(vec![
                Span::styled("Tags:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if file.tags.is_empty() { "—".to_string() } else { file.tags.join(", ") },
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::raw(""),
        ];

        if app.chafa_lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "(image not available — file not on this system)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for l in &app.chafa_lines {
                lines.push(Line::raw(l.clone()));
            }
        }
        lines
    } else {
        vec![Line::raw("No selection")]
    };

    let para = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(" Preview "))
        .wrap(Wrap { trim: false });

    frame.render_widget(para, area);
}

fn draw_filter(frame: &mut Frame, app: &App, area: Rect) {
    let filter_text = if app.filter.is_empty() {
        Span::styled(
            "Type to filter…  |  Enter: preview  |  PgUp/PgDn: page  |  q: quit",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::styled(
            format!("/{}_", app.filter),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )
    };

    let para = Paragraph::new(Line::from(filter_text))
        .block(Block::default().borders(Borders::ALL).title(" Filter "));

    frame.render_widget(para, area);
}
