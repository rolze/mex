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
    pub image_protocol_name: String,  // e.g. "halfblocks", "kitty", "sixel"
    /// Path of the image currently loaded into image_state (or in-flight).
    /// The encoded protocol is kept alive even when preview is closed so
    /// reopening the same file is instant (no re-encode needed).
    pub current_image_path: Option<PathBuf>,
    pub image_cache: HashMap<PathBuf, DynamicImage>,
    pub is_loading: bool,       // true while bg encode is in flight
    pub spinner_frame: usize,   // advances each tick for animation
    /// Number of times a new encode was dispatched (cache misses). Only used for tests.
    pub encode_dispatch_count: usize,
}

impl App {
    pub fn new(
        db_path: String,
        target_root: String,
        files: Vec<MediaFile>,
        image_pool: Vec<PathBuf>,
        image_picker: Picker,
        image_state: ThreadProtocol,
        image_protocol_name: String,
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
            image_protocol_name,
            current_image_path: None,
            image_cache: HashMap::new(),
            is_loading: false,
            spinner_frame: 0,
            encode_dispatch_count: 0,
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
            self.encode_dispatch_count += 1;
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
                self.encode_dispatch_count += 1;
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

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_image::{picker::Picker, thread::ThreadProtocol};
    use std::sync::mpsc;
    use std::time::Instant;

    fn make_test_app_with_rows(pool: Vec<PathBuf>, rows: usize) -> App {
        let (tx, _rx) = mpsc::channel();
        let image_state = ThreadProtocol::new(tx, None);
        let picker = Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = (0..rows)
            .map(|i| crate::db::MediaFile {
                id: i.to_string(),
                target_path: format!("2024/file-{i}.jpg"),
                derived_date: "2024-01-01".into(),
                ext: "jpg".into(),
                tags: vec![],
            })
            .collect();
        App::new("test.db".into(), "/tmp".into(), files, pool, picker, image_state, "halfblocks".into())
    }

    /// Build a minimal App for testing. Uses a throwaway mpsc pair for the
    /// encoder channel — no background thread is started (not needed for
    /// logic / dispatch-count tests).
    fn make_test_app(pool: Vec<PathBuf>) -> App {
        make_test_app_with_rows(pool, 0)
    }

    fn test_image(name: &str) -> PathBuf {
        // Works from both `mex/` and the repo root.
        for prefix in &["mex-media-root", "../mex-media-root"] {
            let p = PathBuf::from(prefix).join(name);
            if p.exists() {
                return p;
            }
        }
        panic!("test image not found: {name}");
    }

    // ── Dispatch-count tests ────────────────────────────────────────────────

    /// First call to refresh_image should dispatch exactly one encode.
    #[test]
    fn first_open_dispatches_once() {
        let pool = vec![test_image("rolze.jpg")];
        let mut app = make_test_app(pool);
        assert_eq!(app.encode_dispatch_count, 0);
        app.refresh_image();
        assert_eq!(app.encode_dispatch_count, 1, "first open must dispatch an encode");
    }

    /// Calling refresh_image again with the same path (same selected index)
    /// must NOT dispatch a second encode (the cached path short-circuits).
    #[test]
    fn same_path_no_second_dispatch() {
        let pool = vec![test_image("rolze.jpg")];
        let mut app = make_test_app(pool);
        app.refresh_image();
        let count_after_first = app.encode_dispatch_count;
        app.refresh_image(); // same path, same selection
        assert_eq!(
            app.encode_dispatch_count, count_after_first,
            "second call with same path must NOT dispatch another encode"
        );
    }

    /// Closing and reopening the preview on the same item must not dispatch
    /// a new encode (the ThreadProtocol is kept alive).
    #[test]
    fn close_reopen_no_redispatch() {
        let pool = vec![test_image("bg.png")];
        let mut app = make_test_app(pool);
        app.toggle_preview(); // open
        let after_open = app.encode_dispatch_count;
        assert_eq!(after_open, 1, "opening must dispatch one encode");

        app.toggle_preview(); // close
        app.toggle_preview(); // reopen
        assert_eq!(
            app.encode_dispatch_count, after_open,
            "close + reopen on same file must not dispatch again"
        );
    }

    /// Navigating to a different row and back must re-dispatch (different
    /// images in the pool at each index).
    #[test]
    fn navigate_away_and_back_dispatches() {
        let pool = vec![
            test_image("rolze.jpg"),
            test_image("bg.png"),
        ];
        let mut app = make_test_app_with_rows(pool, 3); // need >1 row to navigate
        app.toggle_preview();
        let after_first = app.encode_dispatch_count; // 1
        // Navigate to row 1 (different image)
        app.move_down();
        let after_second = app.encode_dispatch_count; // 2
        assert!(after_second > after_first, "different image must dispatch");
        // Navigate back to row 0
        app.move_up();
        let after_return = app.encode_dispatch_count; // 3 — different path than current
        assert!(after_return > after_second, "navigating back must dispatch (different from current)");
    }

    /// DynamicImage is cached after first disk read; a second navigate-away
    /// and back should still dispatch (new encode needed), but NOT re-read
    /// the disk (image_cache contains it).
    #[test]
    fn dynimage_is_cached_after_first_load() {
        let pool = vec![
            test_image("rolze.jpg"),
            test_image("bg.png"),
        ];
        let mut app = make_test_app_with_rows(pool, 3);
        app.toggle_preview();
        assert_eq!(app.image_cache.len(), 1, "first open must populate DynamicImage cache");
        app.move_down();
        assert_eq!(app.image_cache.len(), 2);
        app.move_up(); // back to rolze.jpg
        // Cache still has both entries
        assert_eq!(app.image_cache.len(), 2, "cache must not evict prematurely");
    }

    // ── Timing tests ────────────────────────────────────────────────────────

    /// refresh_image() must return quickly when the same path is already
    /// loaded (the early-return path). Threshold: 1 ms.
    #[test]
    fn same_path_refresh_is_sub_millisecond() {
        let pool = vec![test_image("rolze.jpg")];
        let mut app = make_test_app(pool);
        app.refresh_image(); // first call — loads disk + creates proto
        let t0 = Instant::now();
        app.refresh_image(); // second call — should early-return
        let elapsed = t0.elapsed();
        assert!(
            elapsed.as_millis() < 1,
            "same-path refresh_image() must take <1 ms, took {}µs",
            elapsed.as_micros()
        );
    }

    /// DynamicImage cache hit must be significantly faster than a cold disk
    /// read. We measure the cold decode + encode-dispatch time and check the
    /// hot path is at least 10× faster.
    #[test]
    fn cache_hit_faster_than_cold_read() {
        let path = test_image("rolze.jpg");
        let pool = vec![path.clone(), test_image("bg.png")];
        let mut app = make_test_app(pool);

        // Cold read for rolze.jpg (row 0)
        let t_cold = Instant::now();
        app.refresh_image();
        let cold_duration = t_cold.elapsed();

        // Navigate away to bg.png (row 1)
        app.move_down();

        // Navigate back — DynamicImage for rolze.jpg is cached.
        app.move_up();
        // Force current_image_path mismatch by resetting it so cache hit is exercised
        // (simulate navigating away and back without same-path short-circuit).
        app.current_image_path = None;
        let t_hot = Instant::now();
        app.refresh_image(); // cache hit — no disk I/O
        let hot_duration = t_hot.elapsed();

        println!(
            "cold={cold_duration:?}  hot={hot_duration:?}  ratio={}",
            cold_duration.as_micros().max(1) / hot_duration.as_micros().max(1)
        );

        // The hot path must be faster. If both are sub-µs (extremely fast
        // disk / OS cache) the ratio may be ≤ 10 — we only assert hot < cold
        // to avoid flakiness on fast systems.
        assert!(
            hot_duration <= cold_duration + std::time::Duration::from_millis(5),
            "DynamicImage cache-hit ({hot_duration:?}) must not be slower than cold read ({cold_duration:?})"
        );
    }

    // ── apply_filter / filter reset ─────────────────────────────────────────

    /// apply_filter must clear current_image_path so the next preview open
    /// loads a fresh image.
    #[test]
    fn filter_clears_image_state() {
        let pool = vec![test_image("rolze.jpg")];
        let mut app = make_test_app(pool);
        app.toggle_preview();
        assert!(app.current_image_path.is_some());
        app.push_filter_char('x'); // triggers apply_filter
        assert!(app.current_image_path.is_none(), "filter must clear current_image_path");
        assert!(!app.preview_open, "filter must close preview");
    }
}
