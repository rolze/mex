use crate::config::Config;
use crate::db;
use crate::domain::filter::Filter;
use crate::domain::media::MediaItem;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rusqlite::Connection;

pub enum Mode {
    Normal,
    Filter,
    Command,
    Caption,
}

pub enum View {
    All,
    Import,
    Trash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ZoomLevel {
    Flat = 0,
    Slug = 1,
    Month = 2,
    Year = 3,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ListRow {
    Item(usize),
    GroupSummary {
        level: ZoomLevel,
        key: String,
        start_idx: usize,
        end_idx: usize,
    },
}

pub struct App {
    pub config: Config,
    pub db_conn: Connection,
    pub items: Vec<MediaItem>,
    pub filtered_items: Vec<usize>, // Indices into items
    pub visible_rows: Vec<ListRow>,
    pub global_zoom: ZoomLevel,
    pub expanded_overrides: std::collections::HashSet<String>,
    pub collapsed_overrides: std::collections::HashSet<String>,
    pub selected: std::collections::HashSet<usize>,
    pub cursor_pos: usize,
    pub mode: Mode,
    pub view: View,
    pub filter: Filter,
    pub command_input: String,
    #[allow(dead_code)]
    pub filter_input: String,
    pub status_message: Option<String>,
    pub show_preview: bool,
    pub shift_anchor: Option<usize>,
    pub tag_input: Option<String>,
    pub type_input: Option<String>,
    pub tags_cache: Vec<String>,
    pub types_cache: Vec<String>,
    pub active_completions: Vec<String>,
    pub completion_idx: usize,
    pub list_offset: usize,
    pub list_height: usize,
    pub mpv: crate::services::mpv::MpvContext,
    pub image_cache: std::collections::HashMap<String, ratatui_image::protocol::StatefulProtocol>,
    pub picker: Option<ratatui_image::picker::Picker>,
    pub theme_index: usize,
    pub theme: crate::ui::theme::Theme,
}

impl App {
    pub fn new(config: Config, db_conn: Connection) -> Result<Self> {
        let items = db::media::load_files(&db_conn)?;
        let filtered_items = (0..items.len()).collect();

        let mut app = Self {
            config,
            db_conn,
            items,
            filtered_items,
            visible_rows: Vec::new(),
            global_zoom: ZoomLevel::Flat,
            expanded_overrides: std::collections::HashSet::new(),
            collapsed_overrides: std::collections::HashSet::new(),
            selected: std::collections::HashSet::new(),
            cursor_pos: 0,
            mode: Mode::Normal,
            view: View::All,
            filter: Filter::default(),
            command_input: String::new(),
            filter_input: String::new(),
            status_message: None,
            show_preview: false,
            shift_anchor: None,
            tag_input: None,
            type_input: None,
            tags_cache: Vec::new(),
            types_cache: Vec::new(),
            active_completions: Vec::new(),
            completion_idx: 0,
            list_offset: 0,
            list_height: 10,
            mpv: crate::services::mpv::MpvContext::new(),
            image_cache: std::collections::HashMap::new(),
            picker: None,
            theme_index: 0,
            theme: crate::ui::theme::Theme::ALL[0],
        };
        app.update_caches();
        app.build_visible_rows();
        Ok(app)
    }

    pub fn update_caches(&mut self) {
        if let Ok(tags) = crate::db::tags::load_all_tags(&self.db_conn) {
            let mut all_tags = Vec::new();
            let mut all_types = std::collections::HashSet::new();
            for tag in tags {
                all_tags.push(tag.name);
                if !tag.type_.is_empty() {
                    all_types.insert(tag.type_);
                }
            }
            self.tags_cache = all_tags; // already sorted by DB
            let mut types_vec: Vec<String> = all_types.into_iter().collect();
            types_vec.sort();
            self.types_cache = types_vec;
        }
    }

    pub fn update_completions(&mut self) {
        self.active_completions = if let Some(tag) = &self.tag_input {
            if tag.is_empty() {
                Vec::new()
            } else {
                let prefix = tag.to_lowercase();
                self.tags_cache
                    .iter()
                    .filter(|t| t.to_lowercase().starts_with(&prefix))
                    .cloned()
                    .collect()
            }
        } else if let Some(typ) = &self.type_input {
            if typ.is_empty() {
                Vec::new()
            } else {
                let prefix = typ.to_lowercase();
                self.types_cache
                    .iter()
                    .filter(|t| t.to_lowercase().starts_with(&prefix))
                    .cloned()
                    .collect()
            }
        } else {
            Vec::new()
        };
    }

    pub fn get_current_completions(&self) -> &[String] {
        &self.active_completions
    }

    pub fn get_current_completion(&self) -> Option<String> {
        if self.active_completions.is_empty() {
            None
        } else {
            Some(
                self.active_completions[self.completion_idx % self.active_completions.len()]
                    .clone(),
            )
        }
    }

    pub fn is_collapsed(&self, level: ZoomLevel, key: &str) -> bool {
        let default_collapsed = (level as u8) <= (self.global_zoom as u8);
        if default_collapsed {
            !self.expanded_overrides.contains(key)
        } else {
            self.collapsed_overrides.contains(key)
        }
    }

    pub fn build_visible_rows(&mut self) {
        self.visible_rows.clear();
        let mut i = 0;
        while i < self.filtered_items.len() {
            let idx = self.filtered_items[i];
            let item = &self.items[idx];

            if let Some(year) = item.year_str() {
                if self.is_collapsed(ZoomLevel::Year, year) {
                    let start_i = i;
                    let mut j = i + 1;
                    while j < self.filtered_items.len()
                        && self.items[self.filtered_items[j]].year_str() == Some(year)
                    {
                        j += 1;
                    }
                    self.visible_rows.push(ListRow::GroupSummary {
                        level: ZoomLevel::Year,
                        key: year.to_string(),
                        start_idx: start_i,
                        end_idx: j,
                    });
                    i = j;
                    continue;
                }
            }

            if let Some(month) = item.month_str() {
                if self.is_collapsed(ZoomLevel::Month, month) {
                    let start_i = i;
                    let mut j = i + 1;
                    while j < self.filtered_items.len()
                        && self.items[self.filtered_items[j]].month_str() == Some(month)
                    {
                        j += 1;
                    }
                    self.visible_rows.push(ListRow::GroupSummary {
                        level: ZoomLevel::Month,
                        key: month.to_string(),
                        start_idx: start_i,
                        end_idx: j,
                    });
                    i = j;
                    continue;
                }
            }

            if let Some(slug) = item.slug_str() {
                if self.is_collapsed(ZoomLevel::Slug, slug) {
                    let start_i = i;
                    let mut j = i + 1;
                    while j < self.filtered_items.len()
                        && self.items[self.filtered_items[j]].slug_str() == Some(slug)
                    {
                        j += 1;
                    }
                    self.visible_rows.push(ListRow::GroupSummary {
                        level: ZoomLevel::Slug,
                        key: slug.to_string(),
                        start_idx: start_i,
                        end_idx: j,
                    });
                    i = j;
                    continue;
                }
            }

            self.visible_rows.push(ListRow::Item(idx));
            i += 1;
        }
    }

    pub fn tick(&mut self) {
        for event in self.mpv.poll_events() {
            match event {
                crate::services::mpv::MpvEvent::Ended => {
                    self.mpv_next_video();
                }
            }
        }
    }

    /// Handles a key event and returns true if the app should exit.
    pub fn get_item_idx(&self, pos: usize) -> Option<usize> {
        if let Some(ListRow::Item(idx)) = self.visible_rows.get(pos) {
            Some(*idx)
        } else {
            None
        }
    }

    pub fn get_target_indices(&self) -> Vec<usize> {
        if !self.selected.is_empty() {
            self.selected.iter().copied().collect()
        } else if let Some(ListRow::Item(idx)) = self.visible_rows.get(self.cursor_pos) {
            vec![*idx]
        } else {
            vec![]
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Filter => {
                self.handle_filter_mode(key);
                false
            }
            Mode::Command => self.handle_command_mode(key),
            Mode::Caption => {
                self.handle_caption_mode(key);
                false
            }
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> bool {
        let is_shift = key.modifiers.contains(KeyModifiers::SHIFT);
        if !is_shift {
            self.shift_anchor = None;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return true,
            KeyCode::Char('t') => {
                self.theme_index = (self.theme_index + 1) % crate::ui::theme::Theme::ALL.len();
                self.theme = crate::ui::theme::Theme::ALL[self.theme_index];
                self.status_message = Some(format!("Theme: {}", self.theme.name));
            }
            KeyCode::Char(' ') => {
                if let Some(idx) = self.get_item_idx(self.cursor_pos) {
                    if self.selected.contains(&idx) {
                        self.selected.remove(&idx);
                    } else {
                        self.selected.insert(idx);
                    }
                }
                self.shift_anchor = None;
            }
            KeyCode::Down if self.cursor_pos + 1 < self.visible_rows.len() => {
                let old_pos = self.cursor_pos;
                self.cursor_pos += 1;
                self.check_missing_on_disk();

                if is_shift {
                    if self.shift_anchor.is_none() {
                        self.shift_anchor = Some(old_pos);
                        if let Some(idx) = self.get_item_idx(old_pos) {
                            if self.selected.contains(&idx) {
                                self.selected.remove(&idx);
                            } else {
                                self.selected.insert(idx);
                            }
                        }
                    }
                    if let Some(idx) = self.get_item_idx(self.cursor_pos) {
                        if self.selected.contains(&idx) {
                            self.selected.remove(&idx);
                        } else {
                            self.selected.insert(idx);
                        }
                    }
                }
            }
            KeyCode::Up if self.cursor_pos > 0 => {
                let old_pos = self.cursor_pos;
                self.cursor_pos -= 1;
                self.check_missing_on_disk();

                if is_shift {
                    if self.shift_anchor.is_none() {
                        self.shift_anchor = Some(old_pos);
                        if let Some(idx) = self.get_item_idx(old_pos) {
                            if self.selected.contains(&idx) {
                                self.selected.remove(&idx);
                            } else {
                                self.selected.insert(idx);
                            }
                        }
                    }
                    if let Some(idx) = self.get_item_idx(self.cursor_pos) {
                        if self.selected.contains(&idx) {
                            self.selected.remove(&idx);
                        } else {
                            self.selected.insert(idx);
                        }
                    }
                }
            }
            KeyCode::Left => {
                self.zoom_out();
            }
            KeyCode::Right => {
                self.zoom_in();
            }
            KeyCode::Home => {
                if is_shift {
                    self.shift_home();
                } else {
                    self.jump_home();
                }
            }
            KeyCode::End => {
                if is_shift {
                    self.shift_end();
                } else {
                    self.jump_end();
                }
            }
            KeyCode::PageDown => {
                self.cursor_pos = (self.cursor_pos + self.list_height)
                    .min(self.visible_rows.len().saturating_sub(1));
                self.check_missing_on_disk();
            }
            KeyCode::PageUp => {
                self.cursor_pos = self.cursor_pos.saturating_sub(self.list_height);
                self.check_missing_on_disk();
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_input.clear();
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Filter;
            }

            KeyCode::Char('1') => {
                self.view = View::All;
                self.apply_filter();
            }
            KeyCode::Char('2') => {
                self.view = View::Import;
                self.apply_filter();
            }
            KeyCode::Char('3') => {
                self.view = View::Trash;
                self.apply_filter();
            }
            KeyCode::Char('c') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if let Some(target) = self.get_single_target() {
                        let path = target.source_path.clone();
                        if let Ok(mut child) = std::process::Command::new("wl-copy")
                            .stdin(std::process::Stdio::piped())
                            .spawn()
                        {
                            use std::io::Write;
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(path.as_bytes());
                            }
                        } else if let Ok(mut child) = std::process::Command::new("xclip")
                            .arg("-selection")
                            .arg("clipboard")
                            .stdin(std::process::Stdio::piped())
                            .spawn()
                        {
                            use std::io::Write;
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(path.as_bytes());
                            }
                        }
                        self.status_message = Some(format!("copied: {}", path));
                    }
                } else {
                    self.mode = Mode::Caption;
                    // Pre-fill with current caption if only one item selected/cursor
                    let target = self.get_single_target();
                    if let Some(media) = target {
                        self.command_input = media.caption.clone().unwrap_or_default();
                    } else {
                        self.command_input.clear();
                    }
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let all_selected = self
                    .filtered_items
                    .iter()
                    .all(|&idx| self.selected.contains(&idx));
                if all_selected {
                    for idx in &self.filtered_items {
                        self.selected.remove(idx);
                    }
                } else {
                    for idx in &self.filtered_items {
                        self.selected.insert(*idx);
                    }
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor_pos = (self.cursor_pos + self.list_height)
                    .min(self.visible_rows.len().saturating_sub(1));
                self.check_missing_on_disk();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor_pos = self.cursor_pos.saturating_sub(self.list_height);
                self.check_missing_on_disk();
            }
            KeyCode::Delete => {
                self.set_status(crate::domain::media::Status::Trashed);
            }
            KeyCode::Insert => {
                self.set_status(crate::domain::media::Status::Normal);
            }
            KeyCode::Char('p') => {
                self.mpv_play_current();
            }
            KeyCode::Char('s') => {
                self.mpv.toggle_pause();
            }
            KeyCode::Char('j') => {
                self.mpv_next_video();
            }
            KeyCode::Char('k') => {
                self.mpv_prev_video();
            }
            KeyCode::Enter => {
                self.show_preview = !self.show_preview;
                if self.show_preview {
                    self.check_missing_on_disk();
                }
            }
            KeyCode::Esc => {
                if !self.selected.is_empty() {
                    self.selected.clear();
                } else if self.show_preview {
                    self.show_preview = false;
                } else {
                    self.filter.clear();
                    self.apply_filter();
                }
                self.status_message = None;
            }
            _ => {}
        }
        false
    }

    fn handle_filter_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.filter.clear();
                self.tag_input = None;
                self.type_input = None;
                self.completion_idx = 0;
                self.apply_filter();
            }
            KeyCode::Char('#') if self.type_input.is_some() || self.tag_input.is_none() => {
                self.tag_input = Some(String::new());
                self.type_input = None;
                self.completion_idx = 0;
                self.update_completions();
            }
            KeyCode::Char('@') if self.tag_input.is_some() || self.type_input.is_none() => {
                self.type_input = Some(String::new());
                self.tag_input = None;
                self.completion_idx = 0;
                self.update_completions();
            }
            KeyCode::Char(c) => {
                if let Some(t) = &mut self.tag_input {
                    t.push(c);
                    self.completion_idx = 0;
                    self.update_completions();
                } else if let Some(t) = &mut self.type_input {
                    t.push(c);
                    self.completion_idx = 0;
                    self.update_completions();
                } else {
                    self.filter.text.push(c);
                    self.apply_filter();
                }
            }
            KeyCode::Backspace => {
                if let Some(t) = &mut self.tag_input {
                    if t.is_empty() {
                        self.tag_input = None;
                    } else {
                        t.pop();
                        self.completion_idx = 0;
                    }
                    self.update_completions();
                } else if let Some(t) = &mut self.type_input {
                    if t.is_empty() {
                        self.type_input = None;
                    } else {
                        t.pop();
                        self.completion_idx = 0;
                    }
                    self.update_completions();
                } else if !self.filter.text.is_empty() {
                    self.filter.text.pop();
                    self.apply_filter();
                } else if !self.filter.tags.is_empty() {
                    self.filter.tags.pop();
                    self.apply_filter();
                } else if !self.filter.types.is_empty() {
                    self.filter.types.pop();
                    self.apply_filter();
                }
            }
            KeyCode::Up => {
                if self.completion_idx > 0 {
                    self.completion_idx -= 1;
                } else {
                    let comps = self.get_current_completions();
                    if !comps.is_empty() {
                        self.completion_idx = comps.len() - 1;
                    }
                }
            }
            KeyCode::Down => {
                let comps = self.get_current_completions();
                if !comps.is_empty() {
                    self.completion_idx = (self.completion_idx + 1) % comps.len();
                }
            }
            KeyCode::Tab => {
                if let Some(comp) = self.get_current_completion() {
                    if self.tag_input.is_some() {
                        self.tag_input = Some(comp);
                        self.update_completions();
                    } else if self.type_input.is_some() {
                        self.type_input = Some(comp);
                        self.update_completions();
                    }
                    self.completion_idx = 0;
                }
            }
            KeyCode::Enter => {
                if let Some(t) = self.tag_input.take() {
                    if !t.is_empty() && !self.filter.tags.iter().any(|x| x.eq_ignore_ascii_case(&t))
                    {
                        self.filter.tags.push(t);
                    }
                } else if let Some(t) = self.type_input.take() {
                    if !t.is_empty()
                        && !self.filter.types.iter().any(|x| x.eq_ignore_ascii_case(&t))
                    {
                        self.filter.types.push(t);
                    }
                } else {
                    self.mode = Mode::Normal;
                }
                self.apply_filter();
            }
            _ => {}
        }
    }

    fn handle_command_mode(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => {
                self.command_input.push(c);
            }
            KeyCode::Backspace => {
                self.command_input.pop();
            }
            KeyCode::Enter => {
                let should_exit = self.execute_command();
                self.mode = Mode::Normal;
                return should_exit;
            }
            _ => {}
        }
        false
    }

    fn handle_caption_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => {
                self.command_input.push(c);
            }
            KeyCode::Backspace => {
                self.command_input.pop();
            }
            KeyCode::Enter => {
                self.apply_caption(self.command_input.clone());
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    pub fn apply_filter(&mut self) {
        let text = self.filter.text.to_lowercase();
        let parts: Vec<&str> = text.split('*').collect();

        let active_tags: Vec<String> = self.filter.tags.iter().map(|s| s.to_lowercase()).collect();
        let active_types: Vec<String> =
            self.filter.types.iter().map(|s| s.to_lowercase()).collect();

        self.filtered_items = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                // Check text matches
                let mut matches_text = true;
                if !text.is_empty() {
                    let fname = item.file_name().unwrap_or_default().to_lowercase();
                    let mut current_idx = 0;
                    for part in &parts {
                        if part.is_empty() {
                            continue;
                        }
                        if let Some(pos) = fname[current_idx..].find(part) {
                            current_idx += pos + part.len();
                        } else {
                            matches_text = false;
                            break;
                        }
                    }
                }

                // Check tag match (OR logic internally, AND with text)
                let matches_tags = active_tags.is_empty()
                    || active_tags.iter().any(|t| {
                        item.tags_packed
                            .split('\x1f')
                            .any(|tag| tag.eq_ignore_ascii_case(t))
                    });

                // Check type match
                let matches_types = active_types.is_empty()
                    || active_types.iter().any(|t| {
                        item.tag_types_packed
                            .split('\x1f')
                            .any(|typ| typ.eq_ignore_ascii_case(t))
                    });

                // Check view match
                let matches_view = match self.view {
                    View::All => {
                        item.status == crate::domain::media::Status::Normal
                            || item.status == crate::domain::media::Status::Imported
                    }
                    View::Import => item.status == crate::domain::media::Status::Imported,
                    View::Trash => item.status == crate::domain::media::Status::Trashed,
                };

                if matches_text && matches_tags && matches_types && matches_view {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        self.build_visible_rows();
        self.cursor_pos = 0;
    }

    fn execute_command(&mut self) -> bool {
        let cmd = self.command_input.trim().to_string();
        crate::services::commands::execute(self, &cmd)
    }

    fn jump_home(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        let current_key = self.get_group_key(self.cursor_pos);

        // Find start of current group
        let mut group_start = self.cursor_pos;
        while group_start > 0 && self.get_group_key(group_start - 1) == current_key {
            group_start -= 1;
        }

        if self.cursor_pos == group_start && group_start > 0 {
            // Already at group start, jump to previous group start
            let prev_key = self.get_group_key(group_start - 1);
            let mut prev_start = group_start - 1;
            while prev_start > 0 && self.get_group_key(prev_start - 1) == prev_key {
                prev_start -= 1;
            }
            self.cursor_pos = prev_start;
        } else {
            self.cursor_pos = group_start;
        }
    }

    fn jump_end(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        let current_key = self.get_group_key(self.cursor_pos);

        let mut next_start = self.cursor_pos;
        while next_start < self.visible_rows.len() && self.get_group_key(next_start) == current_key
        {
            next_start += 1;
        }

        if next_start < self.visible_rows.len() {
            self.cursor_pos = next_start;
        }
    }

    fn shift_home(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        let current_key = self.get_group_key(self.cursor_pos);

        let mut group_start = self.cursor_pos;
        while group_start > 0 && self.get_group_key(group_start - 1) == current_key {
            group_start -= 1;
        }

        let range = if self.cursor_pos == group_start && group_start > 0 {
            let prev_key = self.get_group_key(group_start - 1);
            let mut prev_start = group_start - 1;
            while prev_start > 0 && self.get_group_key(prev_start - 1) == prev_key {
                prev_start -= 1;
            }
            self.cursor_pos = prev_start;
            prev_start..=group_start - 1
        } else {
            let old_pos = self.cursor_pos;
            self.cursor_pos = group_start;
            group_start..=old_pos
        };

        self.toggle_range(range);
    }

    fn shift_end(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        let current_key = self.get_group_key(self.cursor_pos);

        let mut group_end = self.cursor_pos;
        while group_end + 1 < self.visible_rows.len()
            && self.get_group_key(group_end + 1) == current_key
        {
            group_end += 1;
        }

        let old_pos = self.cursor_pos;
        let range = old_pos..=group_end;

        if group_end + 1 < self.visible_rows.len() {
            self.cursor_pos = group_end + 1;
        } else {
            self.cursor_pos = group_end;
        }

        self.toggle_range(range);
    }

    fn get_group_key(&self, pos: usize) -> Option<String> {
        self.filtered_items
            .get(pos)
            .and_then(|&idx| self.items.get(idx))
            .and_then(|media| media.group_key())
    }

    fn toggle_range(&mut self, range: std::ops::RangeInclusive<usize>) {
        let all_selected = range.clone().all(|pos| {
            if let Some(idx) = self.get_item_idx(pos) {
                self.selected.contains(&idx)
            } else {
                true // if it's a group summary, pretend it matches to not break all_selected
            }
        });

        for pos in range {
            if let Some(idx) = self.get_item_idx(pos) {
                if all_selected {
                    self.selected.remove(&idx);
                } else {
                    self.selected.insert(idx);
                }
            }
        }
    }

    fn get_single_target(&self) -> Option<&crate::domain::media::MediaItem> {
        if self.selected.len() == 1 {
            let idx = *self.selected.iter().next().unwrap();
            self.items.get(idx)
        } else if self.selected.is_empty() {
            self.filtered_items
                .get(self.cursor_pos)
                .and_then(|&idx| self.items.get(idx))
        } else {
            None
        }
    }

    fn set_status(&mut self, status: crate::domain::media::Status) {
        let targets = if !self.selected.is_empty() {
            self.selected.iter().copied().collect::<Vec<_>>()
        } else if let Some(idx) = self.get_item_idx(self.cursor_pos) {
            vec![idx]
        } else {
            vec![]
        };

        if targets.is_empty() {
            return;
        }

        let ids: Vec<String> = targets
            .iter()
            .filter_map(|&idx| self.items.get(idx).map(|m| m.id.clone()))
            .collect();

        // Update DB
        if let Err(e) = crate::db::media::update_status(&self.db_conn, &ids, status) {
            self.status_message = Some(format!("DB error: {}", e));
            return;
        }

        // Update in-memory
        for idx in targets {
            if let Some(media) = self.items.get_mut(idx) {
                media.status = status;
            }
        }
    }

    fn apply_caption(&mut self, new_caption: String) {
        // Find targets (for caption, similar rules)
        let targets = if !self.selected.is_empty() {
            self.selected.iter().copied().collect::<Vec<_>>()
        } else if let Some(idx) = self.get_item_idx(self.cursor_pos) {
            vec![idx]
        } else {
            vec![]
        };

        if targets.is_empty() {
            return;
        }

        let ids: Vec<String> = targets
            .iter()
            .filter_map(|&idx| self.items.get(idx).map(|m| m.id.clone()))
            .collect();

        let cap_val = if new_caption.is_empty() {
            None
        } else {
            Some(new_caption.as_str())
        };
        if let Err(e) = crate::db::media::update_caption(&self.db_conn, &ids, cap_val) {
            self.status_message = Some(format!("Error saving caption: {}", e));
            return;
        }

        for idx in targets {
            if let Some(media) = self.items.get_mut(idx) {
                media.caption = if new_caption.is_empty() {
                    None
                } else {
                    Some(new_caption.clone())
                };
            }
        }
        self.status_message = Some(format!("Caption applied to {} items", ids.len()));
    }

    fn mpv_play_current(&mut self) {
        let (path, fname) = if let Some(media) = self.get_single_target() {
            let path = if let (Some(root), Some(rel)) =
                (&self.config.target_root, media.relative_path())
            {
                root.join(rel).to_string_lossy().to_string()
            } else {
                media.source_path.clone()
            };
            (path, media.file_name().unwrap_or_default())
        } else {
            return;
        };

        self.mpv.play(&path);
        self.status_message = Some(format!("Playing: {}", fname));
    }

    fn mpv_next_video(&mut self) {
        for i in self.cursor_pos + 1..self.visible_rows.len() {
            if let Some(idx) = self.get_item_idx(i) {
                if let Some(media) = self.items.get(idx) {
                    if media.ext == ".mp4" || media.ext == ".webm" || media.ext == ".mkv" {
                        self.cursor_pos = i;
                        self.mpv_play_current();
                        return;
                    }
                }
            }
        }
    }

    fn mpv_prev_video(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        for i in (0..self.cursor_pos).rev() {
            if let Some(idx) = self.get_item_idx(i) {
                if let Some(media) = self.items.get(idx) {
                    if media.ext == ".mp4" || media.ext == ".webm" || media.ext == ".mkv" {
                        self.cursor_pos = i;
                        self.mpv_play_current();
                        return;
                    }
                }
            }
        }
    }

    pub fn check_missing_on_disk(&mut self) {
        if !self.show_preview {
            return;
        }
        if let Some(idx) = self.get_item_idx(self.cursor_pos) {
            // Need a separate block to borrow `items` mutably since we might need to modify `db_conn` later.
            let needs_update = {
                if let Some(media) = self.items.get(idx) {
                    let path = if let (Some(root), Some(rel)) =
                        (&self.config.target_root, media.relative_path())
                    {
                        root.join(rel)
                    } else {
                        std::path::PathBuf::from(&media.source_path)
                    };
                    let exists = path.exists();
                    if exists == media.missing_on_disk {
                        Some((media.id.clone(), !exists))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((id, missing)) = needs_update {
                if let Some(media) = self.items.get_mut(idx) {
                    media.missing_on_disk = missing;
                }
                let val = if missing { 1 } else { 0 };
                let _ = self.db_conn.execute(
                    "UPDATE media SET missing_on_disk = ?1 WHERE id = ?2",
                    rusqlite::params![val, id],
                );
            }
        }
    }

    pub fn zoom_out(&mut self) {
        if self.filtered_items.is_empty() || self.visible_rows.is_empty() {
            return;
        }

        let row = self.visible_rows.get(self.cursor_pos).cloned().unwrap();

        let mut target_level = ZoomLevel::Flat;
        let mut target_key = String::new();
        self.status_message = None;

        match row {
            ListRow::Item(idx) => {
                let item = &self.items[idx];
                if let Some(slug) = item.slug_str() {
                    target_level = ZoomLevel::Slug;
                    target_key = slug.to_string();
                    if self.global_zoom >= ZoomLevel::Slug && self.expanded_overrides.contains(slug)
                    {
                        self.expanded_overrides.remove(slug);
                        self.collapsed_overrides.insert(slug.to_string());
                    } else if self.global_zoom < ZoomLevel::Slug {
                        self.global_zoom = ZoomLevel::Slug;
                        self.expanded_overrides.clear();
                        self.collapsed_overrides.clear();
                        self.status_message =
                            Some("Grouped by Slug. Left to group by Month.".to_string());
                    } else {
                        self.collapsed_overrides.insert(slug.to_string());
                    }
                }
            }
            ListRow::GroupSummary {
                level, start_idx, ..
            } => {
                let item = &self.items[self.filtered_items[start_idx]];
                if level == ZoomLevel::Slug {
                    if let Some(month) = item.month_str() {
                        target_level = ZoomLevel::Month;
                        target_key = month.to_string();
                        if self.global_zoom >= ZoomLevel::Month
                            && self.expanded_overrides.contains(month)
                        {
                            self.expanded_overrides.remove(month);
                            self.collapsed_overrides.insert(month.to_string());
                        } else if self.global_zoom < ZoomLevel::Month {
                            self.global_zoom = ZoomLevel::Month;
                            self.expanded_overrides.clear();
                            self.collapsed_overrides.clear();
                            self.status_message =
                                Some("Grouped by Month. Left to group by Year.".to_string());
                        } else {
                            self.collapsed_overrides.insert(month.to_string());
                        }
                    }
                } else if level == ZoomLevel::Month {
                    if let Some(year) = item.year_str() {
                        target_level = ZoomLevel::Year;
                        target_key = year.to_string();
                        if self.global_zoom >= ZoomLevel::Year
                            && self.expanded_overrides.contains(year)
                        {
                            self.expanded_overrides.remove(year);
                            self.collapsed_overrides.insert(year.to_string());
                        } else if self.global_zoom < ZoomLevel::Year {
                            self.global_zoom = ZoomLevel::Year;
                            self.expanded_overrides.clear();
                            self.collapsed_overrides.clear();
                            self.status_message =
                                Some("Grouped by Year. Maximum zoom out.".to_string());
                        } else {
                            self.collapsed_overrides.insert(year.to_string());
                        }
                    }
                } else if level == ZoomLevel::Year {
                    target_level = ZoomLevel::Year;
                    target_key = item.year_str().unwrap_or("").to_string();
                    if self.global_zoom < ZoomLevel::Year {
                        self.global_zoom = ZoomLevel::Year;
                        self.expanded_overrides.clear();
                        self.collapsed_overrides.clear();
                        self.status_message = Some("Grouped by Year globally.".to_string());
                    }
                }
            }
        }

        if self.status_message.is_none() && !target_key.is_empty() {
            let msg = match target_level {
                ZoomLevel::Year => format!("Collapsed year: {}", target_key),
                ZoomLevel::Month => format!("Collapsed month: {}", target_key),
                ZoomLevel::Slug => format!("Collapsed slug/day: {}", target_key),
                _ => format!("Collapsed {}", target_key),
            };
            self.status_message = Some(msg);
        }

        self.build_visible_rows();

        if !target_key.is_empty() {
            if let Some(pos) = self.visible_rows.iter().position(|r| match r {
                ListRow::GroupSummary { level, key, .. } => {
                    *level == target_level && key == &target_key
                }
                _ => false,
            }) {
                self.cursor_pos = pos;
            }
        }
    }

    pub fn zoom_in(&mut self) {
        if self.filtered_items.is_empty() || self.visible_rows.is_empty() {
            return;
        }

        let row = self.visible_rows.get(self.cursor_pos).cloned().unwrap();
        let target_idx = match row {
            ListRow::GroupSummary {
                level,
                key,
                start_idx,
                ..
            } => {
                // It's a collapsed group. Expand it.
                self.expanded_overrides.insert(key.clone());
                self.collapsed_overrides.remove(&key);
                let msg = match level {
                    ZoomLevel::Year => format!("Expanded year {}: showing months", key),
                    ZoomLevel::Month => format!("Expanded month {}: showing slugs/days", key),
                    ZoomLevel::Slug => format!("Expanded slug/day {}: showing items", key),
                    _ => "Expanded group".to_string(),
                };
                self.status_message = Some(msg);
                Some(self.filtered_items[start_idx])
            }
            ListRow::Item(idx) => {
                // We are on an item. Cascading zoom in.
                let item = &self.items[idx];
                let current_month = item.month_str().unwrap_or("");
                let current_year = item.year_str().unwrap_or("");

                // 1. Check if any slug in current_month is collapsed
                let mut has_collapsed_slugs = false;
                for other_idx in &self.filtered_items {
                    let other = &self.items[*other_idx];
                    if other.month_str() == Some(current_month) {
                        if let Some(slug) = other.slug_str() {
                            if self.is_collapsed(ZoomLevel::Slug, slug) {
                                has_collapsed_slugs = true;
                                break;
                            }
                        }
                    }
                }

                if has_collapsed_slugs {
                    // Expand all slugs in current month
                    for other_idx in &self.filtered_items {
                        let other = &self.items[*other_idx];
                        if other.month_str() == Some(current_month) {
                            if let Some(slug) = other.slug_str() {
                                self.expanded_overrides.insert(slug.to_string());
                                self.collapsed_overrides.remove(slug);
                            }
                        }
                    }
                    self.status_message = Some("Expanded all items in Month.".to_string());
                } else {
                    // 2. Check if any month in current_year is collapsed
                    let mut has_collapsed_months = false;
                    for other_idx in &self.filtered_items {
                        let other = &self.items[*other_idx];
                        if other.year_str() == Some(current_year) {
                            if let Some(month) = other.month_str() {
                                if self.is_collapsed(ZoomLevel::Month, month) {
                                    has_collapsed_months = true;
                                    break;
                                }
                            }
                        }
                    }

                    if has_collapsed_months {
                        // Expand all months in current year
                        for other_idx in &self.filtered_items {
                            let other = &self.items[*other_idx];
                            if other.year_str() == Some(current_year) {
                                if let Some(month) = other.month_str() {
                                    self.expanded_overrides.insert(month.to_string());
                                    self.collapsed_overrides.remove(month);
                                }
                            }
                        }
                        self.status_message = Some("Expanded all months in Year.".to_string());
                    } else {
                        // 3. Expand everything globally
                        self.global_zoom = ZoomLevel::Flat;
                        self.expanded_overrides.clear();
                        self.collapsed_overrides.clear();
                        self.status_message = Some("Fully expanded globally.".to_string());
                    }
                }
                Some(idx)
            }
        };

        self.build_visible_rows();

        if let Some(target_idx) = target_idx {
            if let Some(pos) = self.visible_rows.iter().position(|r| match r {
                ListRow::Item(i) => *i == target_idx,
                ListRow::GroupSummary { start_idx, .. } => {
                    self.filtered_items[*start_idx] == target_idx
                }
            }) {
                self.cursor_pos = pos;
            }
        }
    }
}
