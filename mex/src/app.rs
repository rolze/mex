use crate::db::MediaFile;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, thread::ThreadProtocol};
use std::{collections::HashMap, path::PathBuf};

const CACHE_MAX: usize = 30;

pub struct App {
    pub db_path: String,
    pub target_root: String,
    pub all_files: Vec<MediaFile>,
    pub filtered: Vec<MediaFile>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub filter: String,
    pub preview_open: bool,
    pub list_height: usize,     // updated each frame
    pub list_area: Rect,        // updated each frame
    // Image display
    pub image_pool: Vec<PathBuf>,
    pub image_picker: Picker,
    pub image_state: ThreadProtocol,
    /// Path of the image currently loaded into image_state (or in-flight).
    /// The encoded protocol is kept alive even when preview is closed so
    /// reopening the same file is instant (no re-encode needed).
    pub current_image_path: Option<PathBuf>,
    pub image_cache: HashMap<PathBuf, DynamicImage>,
    pub is_loading: bool,       // true while bg encode is in flight
    pub spinner_frame: usize,   // advances each tick for animation
}

impl App {
    pub fn new(
        db_path: String,
        target_root: String,
        files: Vec<MediaFile>,
        image_pool: Vec<PathBuf>,
        image_picker: Picker,
        image_state: ThreadProtocol,
    ) -> Self {
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
            list_height: 20,
            list_area: Rect::default(),
            image_pool,
            image_picker,
            image_state,
            current_image_path: None,
            image_cache: HashMap::new(),
            is_loading: false,
            spinner_frame: 0,
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
            self.ensure_visible();
            if self.preview_open { self.refresh_image(); }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
            if self.preview_open { self.refresh_image(); }
        }
    }

    pub fn jump_top(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
        if self.preview_open { self.refresh_image(); }
    }

    pub fn jump_bottom(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
            self.ensure_visible();
            if self.preview_open { self.refresh_image(); }
        }
    }

    pub fn half_page_down(&mut self) {
        let step = self.list_height / 2;
        self.selected = (self.selected + step).min(self.filtered.len().saturating_sub(1));
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
    }

    pub fn half_page_up(&mut self) {
        let step = self.list_height / 2;
        self.selected = self.selected.saturating_sub(step);
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
    }

    pub fn page_down(&mut self) {
        let step = self.list_height.max(1);
        self.selected = (self.selected + step).min(self.filtered.len().saturating_sub(1));
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
    }

    pub fn page_up(&mut self) {
        let step = self.list_height.max(1);
        self.selected = self.selected.saturating_sub(step);
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
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
        // Discard protocol — new filter means a new image will be shown.
        self.image_state.empty_protocol();
        self.current_image_path = None;
        self.is_loading = false;
        self.preview_open = false;
    }

    pub fn toggle_preview(&mut self) {
        self.preview_open = !self.preview_open;
        if self.preview_open {
            self.refresh_image();
        }
        // On close: keep image_state alive so reopening is instant (no re-encode).
    }

    /// Load image for current selection and send to the background encoder thread.
    /// Skips everything if the same image is already loaded/in-flight (instant reopen).
    pub fn refresh_image(&mut self) {
        if self.image_pool.is_empty() {
            self.image_state.empty_protocol();
            self.current_image_path = None;
            self.is_loading = false;
            return;
        }
        let path = self.image_pool[self.selected % self.image_pool.len()].clone();

        // Already loaded (or in-flight) for this path — nothing to do.
        // StatefulImage will handle terminal-resize re-encodes automatically.
        if self.current_image_path.as_ref() == Some(&path) {
            return;
        }

        // Cache hit: clone decoded pixels, hand to bg thread for encode. No disk I/O.
        if let Some(cached) = self.image_cache.get(&path) {
            let proto: StatefulProtocol = self.image_picker.new_resize_protocol(cached.clone());
            self.image_state.replace_protocol(proto);
            self.current_image_path = Some(path);
            self.is_loading = true;
            return;
        }

        // Cache miss: read from disk, cache, then encode.
        match image::ImageReader::open(&path)
            .and_then(|r| r.decode().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
        {
            Ok(dyn_img) => {
                if self.image_cache.len() >= CACHE_MAX {
                    let victims: Vec<PathBuf> = self.image_cache.keys()
                        .filter(|k| *k != &path)
                        .take(CACHE_MAX / 3)
                        .cloned()
                        .collect();
                    for k in victims { self.image_cache.remove(&k); }
                }
                self.image_cache.insert(path.clone(), dyn_img.clone());
                let proto: StatefulProtocol = self.image_picker.new_resize_protocol(dyn_img);
                self.image_state.replace_protocol(proto);
                self.current_image_path = Some(path);
                self.is_loading = true;
            }
            Err(_) => {
                self.image_state.empty_protocol();
                self.current_image_path = None;
                self.is_loading = false;
            }
        }
    }

    /// Called when the background thread finishes encoding an image.
    pub fn on_encode_done(&mut self, response: ratatui_image::thread::ResizeResponse) {
        if self.image_state.update_resized_protocol(response) {
            self.is_loading = false;
        }
    }

    /// Advance spinner animation — call once per event-loop tick.
    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    pub fn selected_file(&self) -> Option<&MediaFile> {
        self.filtered.get(self.selected)
    }

    pub fn select_at_row(&mut self, row: u16) -> bool {
        let inner_top = self.list_area.y + 1;
        let inner_bottom = self.list_area.y + self.list_area.height.saturating_sub(1);
        if row < inner_top || row >= inner_bottom {
            return false;
        }
        let idx = self.scroll_offset + (row - inner_top) as usize;
        if idx < self.filtered.len() {
            self.selected = idx;
            if self.preview_open { self.refresh_image(); }
        }
        true
    }
}
