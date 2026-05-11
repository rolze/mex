use crate::db::MediaFile;
use ratatui::layout::Rect;

pub struct App {
    pub db_path: String,
    pub target_root: String,
    pub all_files: Vec<MediaFile>,
    pub filtered: Vec<MediaFile>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub filter: String,
    pub preview_open: bool,
    pub chafa_lines: Vec<String>,
    pub list_height: usize, // updated each frame
    pub list_area: Rect,    // bounding box of the list widget, updated each frame
}

impl App {
    pub fn new(db_path: String, target_root: String, files: Vec<MediaFile>) -> Self {
        let filtered = files.clone();
        Self {
            db_path,
            target_root,
            all_files: files,
            filtered,
            selected: 0,
            scroll_offset: 0,
            filter: String::new(),
            preview_open: false,
            chafa_lines: vec![],
            list_height: 20,
            list_area: Rect::default(),
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    pub fn jump_top(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn jump_bottom(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
            self.ensure_visible();
        }
    }

    pub fn half_page_down(&mut self) {
        let step = self.list_height / 2;
        self.selected = (self.selected + step).min(self.filtered.len().saturating_sub(1));
        self.ensure_visible();
    }

    pub fn half_page_up(&mut self) {
        let step = self.list_height / 2;
        self.selected = self.selected.saturating_sub(step);
        self.ensure_visible();
    }

    pub fn page_down(&mut self) {
        let step = self.list_height.max(1);
        self.selected = (self.selected + step).min(self.filtered.len().saturating_sub(1));
        self.ensure_visible();
    }

    pub fn page_up(&mut self) {
        let step = self.list_height.max(1);
        self.selected = self.selected.saturating_sub(step);
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.list_height {
            self.scroll_offset = self.selected + 1 - self.list_height;
        }
    }

    pub fn push_filter_char(&mut self, c: char) {
        self.filter.push(c);
        self.apply_filter();
    }

    pub fn pop_filter_char(&mut self) {
        self.filter.pop();
        self.apply_filter();
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        let needle = self.filter.to_lowercase();
        self.filtered = if needle.is_empty() {
            self.all_files.clone()
        } else {
            self.all_files
                .iter()
                .filter(|f| {
                    f.target_path.to_lowercase().contains(&needle)
                        || f.tags.iter().any(|t| t.to_lowercase().contains(&needle))
                })
                .cloned()
                .collect()
        };
        self.selected = 0;
        self.scroll_offset = 0;
        self.chafa_lines.clear();
        self.preview_open = false;
    }

    pub fn toggle_preview(&mut self) {
        self.preview_open = !self.preview_open;
        if self.preview_open {
            self.refresh_preview();
        } else {
            self.chafa_lines.clear();
        }
    }

    pub fn refresh_preview(&mut self) {
        self.chafa_lines.clear();
        if let Some(file) = self.filtered.get(self.selected) {
            let abs = format!("{}/{}", self.target_root.trim_end_matches('/'), file.target_path);
            if std::path::Path::new(&abs).exists() {
                // Try chafa
                if let Ok(out) = std::process::Command::new("chafa")
                    .args(["--size", "40x20", "--colors", "256", &abs])
                    .output()
                {
                    let text = String::from_utf8_lossy(&out.stdout);
                    self.chafa_lines = text.lines().map(|l| l.to_string()).collect();
                }
            }
        }
    }

    pub fn selected_file(&self) -> Option<&MediaFile> {
        self.filtered.get(self.selected)
    }

    /// Select the file at a terminal row coordinate (from a mouse click).
    /// `row` is the absolute terminal row. Returns true if the click was inside the list.
    pub fn select_at_row(&mut self, row: u16) -> bool {
        let inner_top = self.list_area.y + 1; // +1 for top border
        let inner_bottom = self.list_area.y + self.list_area.height.saturating_sub(1);
        if row < inner_top || row >= inner_bottom {
            return false;
        }
        let idx = self.scroll_offset + (row - inner_top) as usize;
        if idx < self.filtered.len() {
            self.selected = idx;
        }
        true
    }
}
