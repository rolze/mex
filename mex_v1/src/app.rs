use crate::config::Config;
use crate::db;
use crate::domain::media::MediaItem;
use crate::domain::filter::Filter;
use anyhow::Result;
use rusqlite::Connection;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

pub struct App {
    pub config: Config,
    pub db_conn: Connection,
    pub items: Vec<MediaItem>,
    pub filtered_items: Vec<usize>, // Indices into items
    pub selected: std::collections::HashSet<usize>,
    pub cursor_pos: usize,
    pub mode: Mode,
    pub view: View,
    pub filter: Filter,
    pub command_input: String,
    pub filter_input: String,
    pub status_message: Option<String>,
    pub show_preview: bool,
    pub shift_anchor: Option<usize>,
    pub tag_input: Option<String>,
    pub type_input: Option<String>,
    pub tags_cache: Vec<String>,
    pub types_cache: Vec<String>,
}

impl App {
    pub fn new(config: Config, db_conn: Connection) -> Result<Self> {
        let items = db::media::load_files(&db_conn)?;
        let filtered_items = (0..items.len()).collect();
        
        Ok(Self {
            config,
            db_conn,
            items,
            filtered_items,
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
        })
    }

    /// Handles a key event and returns true if the app should exit.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Filter => {
                self.handle_filter_mode(key);
                false
            }
            Mode::Command => {
                self.handle_command_mode(key);
                false
            }
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
            KeyCode::Char(' ') => {
                if self.cursor_pos < self.filtered_items.len() {
                    let idx = self.filtered_items[self.cursor_pos];
                    if self.selected.contains(&idx) {
                        self.selected.remove(&idx);
                    } else {
                        self.selected.insert(idx);
                    }
                }
                self.shift_anchor = None;
            }
            KeyCode::Down => {
                if self.cursor_pos + 1 < self.filtered_items.len() {
                    let old_pos = self.cursor_pos;
                    self.cursor_pos += 1;
                    
                    if is_shift {
                        if self.shift_anchor.is_none() {
                            self.shift_anchor = Some(old_pos);
                            let idx = self.filtered_items[old_pos];
                            if self.selected.contains(&idx) {
                                self.selected.remove(&idx);
                            } else {
                                self.selected.insert(idx);
                            }
                        }
                        let idx = self.filtered_items[self.cursor_pos];
                        if self.selected.contains(&idx) {
                            self.selected.remove(&idx);
                        } else {
                            self.selected.insert(idx);
                        }
                    }
                }
            }
            KeyCode::Up => {
                if self.cursor_pos > 0 {
                    let old_pos = self.cursor_pos;
                    self.cursor_pos -= 1;
                    
                    if is_shift {
                        if self.shift_anchor.is_none() {
                            self.shift_anchor = Some(old_pos);
                            let idx = self.filtered_items[old_pos];
                            if self.selected.contains(&idx) {
                                self.selected.remove(&idx);
                            } else {
                                self.selected.insert(idx);
                            }
                        }
                        let idx = self.filtered_items[self.cursor_pos];
                        if self.selected.contains(&idx) {
                            self.selected.remove(&idx);
                        } else {
                            self.selected.insert(idx);
                        }
                    }
                }
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
                self.cursor_pos = (self.cursor_pos + 10).min(self.filtered_items.len().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.cursor_pos = self.cursor_pos.saturating_sub(10);
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_input.clear();
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Filter;
            }
            KeyCode::Char('t') => {
                self.set_status(crate::domain::media::Status::Trashed);
            }
            KeyCode::Char('k') => {
                self.set_status(crate::domain::media::Status::Normal);
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
                self.mode = Mode::Caption;
                // Pre-fill with current caption if only one item selected/cursor
                let target = self.get_single_target();
                if let Some(media) = target {
                    self.command_input = media.caption.clone().unwrap_or_default();
                } else {
                    self.command_input.clear();
                }
            }
            KeyCode::Enter => {
                self.show_preview = !self.show_preview;
            }
            _ => {}
        }
        false
    }

    fn handle_filter_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                // keep filter
            }
            KeyCode::Char('#') if self.tag_input.is_none() && self.type_input.is_none() => {
                self.tag_input = Some(String::new());
                self.type_input = None;
            }
            KeyCode::Char('@') if self.type_input.is_none() && self.tag_input.is_none() => {
                self.type_input = Some(String::new());
                self.tag_input = None;
            }
            KeyCode::Char(c) => {
                if let Some(t) = &mut self.tag_input {
                    t.push(c);
                } else if let Some(t) = &mut self.type_input {
                    t.push(c);
                } else {
                    self.filter.text.push(c);
                }
                self.apply_filter();
            }
            KeyCode::Backspace => {
                if let Some(t) = &mut self.tag_input {
                    if t.is_empty() {
                        self.tag_input = None;
                    } else {
                        t.pop();
                    }
                } else if let Some(t) = &mut self.type_input {
                    if t.is_empty() {
                        self.type_input = None;
                    } else {
                        t.pop();
                    }
                } else if !self.filter.text.is_empty() {
                    self.filter.text.pop();
                } else if !self.filter.tags.is_empty() {
                    self.filter.tags.pop();
                } else if !self.filter.types.is_empty() {
                    self.filter.types.pop();
                }
                self.apply_filter();
            }
            KeyCode::Enter => {
                if let Some(t) = self.tag_input.take() {
                    if !t.is_empty() && !self.filter.tags.contains(&t) {
                        self.filter.tags.push(t);
                    }
                } else if let Some(t) = self.type_input.take() {
                    if !t.is_empty() && !self.filter.types.contains(&t) {
                        self.filter.types.push(t);
                    }
                }
                self.apply_filter();
            }
            _ => {}
        }
    }

    fn handle_command_mode(&mut self, key: KeyEvent) {
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
                self.execute_command();
                self.mode = Mode::Normal;
            }
            _ => {}
        }
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

    fn apply_filter(&mut self) {
        let text = self.filter.text.to_lowercase();
        let parts: Vec<&str> = text.split('*').collect();
        
        let active_tags: Vec<String> = self.filter.tags.iter().map(|s| s.to_lowercase()).collect();
        let active_types: Vec<String> = self.filter.types.iter().map(|s| s.to_lowercase()).collect();

        self.filtered_items = self.items.iter().enumerate().filter_map(|(i, item)| {
            // Check text matches
            let fname = item.file_name().unwrap_or_default().to_lowercase();
            let mut matches_text = true;
            if !text.is_empty() {
                let mut current_idx = 0;
                for part in &parts {
                    if part.is_empty() { continue; }
                    if let Some(pos) = fname[current_idx..].find(part) {
                        current_idx += pos + part.len();
                    } else {
                        matches_text = false;
                        break;
                    }
                }
            }

            // Check tag match (OR logic internally, AND with text)
            let matches_tags = active_tags.is_empty() || active_tags.iter().any(|t| {
                item.tags_packed.to_lowercase().contains(&format!("{}\x1f", t)) || 
                item.tags_packed.to_lowercase().ends_with(t) ||
                item.tags_packed.to_lowercase() == *t
            });

            // Check type match
            let matches_types = active_types.is_empty() || active_types.iter().any(|t| {
                item.tag_types_packed.to_lowercase().contains(&format!("{}\x1f", t)) ||
                item.tag_types_packed.to_lowercase().ends_with(t) ||
                item.tag_types_packed.to_lowercase() == *t
            });

            // Check view match
            let matches_view = match self.view {
                View::All => item.status == crate::domain::media::Status::Normal || item.status == crate::domain::media::Status::Imported,
                View::Import => item.status == crate::domain::media::Status::Imported,
                View::Trash => item.status == crate::domain::media::Status::Trashed,
            };

            if matches_text && matches_tags && matches_types && matches_view {
                Some(i)
            } else {
                None
            }
        }).collect();
        
        self.cursor_pos = 0;
    }

    fn execute_command(&mut self) {
        let cmd = self.command_input.trim().to_string();
        crate::services::commands::execute(self, &cmd);
    }

    fn jump_home(&mut self) {
        if self.filtered_items.is_empty() { return; }
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
        if self.filtered_items.is_empty() { return; }
        let current_key = self.get_group_key(self.cursor_pos);
        
        let mut next_start = self.cursor_pos;
        while next_start < self.filtered_items.len() && self.get_group_key(next_start) == current_key {
            next_start += 1;
        }

        if next_start < self.filtered_items.len() {
            self.cursor_pos = next_start;
        }
    }

    fn shift_home(&mut self) {
        if self.filtered_items.is_empty() { return; }
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
        if self.filtered_items.is_empty() { return; }
        let current_key = self.get_group_key(self.cursor_pos);
        
        let mut group_end = self.cursor_pos;
        while group_end + 1 < self.filtered_items.len() && self.get_group_key(group_end + 1) == current_key {
            group_end += 1;
        }

        let old_pos = self.cursor_pos;
        let range = old_pos..=group_end;

        if group_end + 1 < self.filtered_items.len() {
            self.cursor_pos = group_end + 1;
        } else {
            self.cursor_pos = group_end;
        }

        self.toggle_range(range);
    }

    fn get_group_key(&self, pos: usize) -> Option<String> {
        self.filtered_items.get(pos)
            .and_then(|&idx| self.items.get(idx))
            .and_then(|media| media.group_key())
    }

    fn toggle_range(&mut self, range: std::ops::RangeInclusive<usize>) {
        let all_selected = range.clone().all(|pos| {
            let idx = self.filtered_items[pos];
            self.selected.contains(&idx)
        });

        for pos in range {
            let idx = self.filtered_items[pos];
            if all_selected {
                self.selected.remove(&idx);
            } else {
                self.selected.insert(idx);
            }
        }
    }

    fn get_single_target(&self) -> Option<&crate::domain::media::MediaItem> {
        if self.selected.len() == 1 {
            let idx = *self.selected.iter().next().unwrap();
            self.items.get(idx)
        } else if self.selected.is_empty() {
            self.filtered_items.get(self.cursor_pos).and_then(|&idx| self.items.get(idx))
        } else {
            None
        }
    }

    fn set_status(&mut self, status: crate::domain::media::Status) {
        let targets = if !self.selected.is_empty() {
            self.selected.iter().copied().collect::<Vec<_>>()
        } else if let Some(&idx) = self.filtered_items.get(self.cursor_pos) {
            vec![idx]
        } else {
            vec![]
        };

        if targets.is_empty() { return; }

        let ids: Vec<String> = targets.iter().filter_map(|&idx| self.items.get(idx).map(|m| m.id.clone())).collect();
        
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
        } else if let Some(&idx) = self.filtered_items.get(self.cursor_pos) {
            vec![idx]
        } else {
            vec![]
        };

        if targets.is_empty() { return; }
        
        let ids: Vec<String> = targets.iter().filter_map(|&idx| self.items.get(idx).map(|m| m.id.clone())).collect();

        // Let's implement caption DB update later. For now, we update in-memory to test.
        for idx in targets {
            if let Some(media) = self.items.get_mut(idx) {
                media.caption = if new_caption.is_empty() { None } else { Some(new_caption.clone()) };
            }
        }
        self.status_message = Some(format!("Caption applied to {} items", ids.len()));
    }
}
