use crate::db::MediaFile;
use image::DynamicImage;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, thread::ThreadProtocol};
use std::{collections::{HashMap, HashSet}, path::PathBuf};

const CACHE_MAX: usize = 30;
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp"];

pub struct App {
    pub db_path: String,
    pub target_root: String,
    pub all_files: Vec<MediaFile>,
    pub filtered: Vec<MediaFile>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub filter: String,
    /// Active command being typed (`:q`, etc.). `None` = search/normal mode.
    pub command: Option<String>,
    pub preview_open: bool,
    pub list_height: usize,     // updated each frame
    // Image display
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
    /// Set to true by execute_command when the user requests quit.
    pub quit: bool,
    /// Indices (into `filtered`) of explicitly selected files.
    pub selection: HashSet<usize>,
    /// Tracks the last item landed on by a Shift-Up/Down move and its direction
    /// (true = down). When a subsequent Shift move continues in the same direction
    /// from the same position, the current item is NOT toggled again (it was already
    /// toggled as "landed" by the previous step).
    pub shift_last_landed: Option<(usize, bool)>,
}

impl App {
    pub fn new(
        db_path: String,
        target_root: String,
        files: Vec<MediaFile>,
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
            command: None,
            preview_open: false,
            list_height: 20,
            image_picker,
            image_state,
            image_protocol_name,
            current_image_path: None,
            image_cache: HashMap::new(),
            is_loading: false,
            spinner_frame: 0,
            encode_dispatch_count: 0,
            quit: false,
            selection: HashSet::new(),
            shift_last_landed: None,
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
            self.shift_last_landed = None;
            self.ensure_visible();
            if self.preview_open { self.refresh_image(); }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.shift_last_landed = None;
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

    // ── Command mode ────────────────────────────────────────────────────────

    /// Enter `:` command mode. Typing a command string and pressing Enter
    /// executes it. All letters are otherwise reserved for live search.
    pub fn enter_command_mode(&mut self) {
        self.command = Some(String::new());
    }

    pub fn push_command_char(&mut self, c: char) {
        if let Some(ref mut cmd) = self.command {
            cmd.push(c);
        }
    }

    /// Pop last char from command buffer; cancel command mode if buffer is empty.
    pub fn pop_command_char(&mut self) {
        match self.command {
            Some(ref mut cmd) if !cmd.is_empty() => { cmd.pop(); }
            _ => self.command = None,
        }
    }

    pub fn cancel_command(&mut self) {
        self.command = None;
    }

    /// Execute the current command. Sets `self.quit` for `:q` / `:quit`.
    /// Clears command mode regardless of outcome.
    pub fn execute_command(&mut self) {
        let cmd = self.command.take().unwrap_or_default();
        match cmd.trim() {
            "q" | "quit" => self.quit = true,
            _ => {} // unknown command — silently ignore for now
        }
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
        self.selection.clear();
        self.shift_last_landed = None;
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
    /// Non-image files (audio, video) or missing files gracefully clear the preview.
    pub fn refresh_image(&mut self) {
        // Build absolute path from target_root + selected file's target_path.
        let path = match self.filtered.get(self.selected) {
            Some(f) if !self.target_root.is_empty() => {
                PathBuf::from(&self.target_root).join(&f.target_path)
            }
            _ => {
                self.image_state.empty_protocol();
                self.current_image_path = None;
                self.is_loading = false;
                return;
            }
        };

        // Skip non-image files rather than attempting (and failing) to decode them.
        let is_image = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| IMAGE_EXTS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false);
        if !is_image || !path.exists() {
            self.image_state.empty_protocol();
            self.current_image_path = None;
            self.is_loading = false;
            return;
        }

        // Already loaded (or in-flight) for this path — nothing to do.
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

    // ── Selection ────────────────────────────────────────────────────────────

    /// Toggle the current cursor row in/out of the selection set.
    pub fn toggle_selection(&mut self) {
        if self.selection.contains(&self.selected) {
            self.selection.remove(&self.selected);
        } else {
            self.selection.insert(self.selected);
        }
    }

    /// Clear all selected files.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.shift_last_landed = None;
    }

    /// Move cursor up, toggling both the item you leave and the item you land on.
    /// Exception: if this continues a Shift-Up sweep (same direction, no gap),
    /// the item you leave is NOT toggled again — it was already toggled when you
    /// landed here in the previous step.
    pub fn extend_selection_up(&mut self) {
        if self.selected > 0 {
            let continuing = self.shift_last_landed == Some((self.selected, false));
            if !continuing {
                self.toggle_index(self.selected);
            }
            self.selected -= 1;
            self.ensure_visible();
            if self.preview_open { self.refresh_image(); }
            self.toggle_index(self.selected);
            self.shift_last_landed = Some((self.selected, false));
        }
    }

    /// Move cursor down, toggling both the item you leave and the item you land on.
    /// Exception: if this continues a Shift-Down sweep (same direction, no gap),
    /// the item you leave is NOT toggled again — it was already toggled when you
    /// landed here in the previous step.
    pub fn extend_selection_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            let continuing = self.shift_last_landed == Some((self.selected, true));
            if !continuing {
                self.toggle_index(self.selected);
            }
            self.selected += 1;
            self.ensure_visible();
            if self.preview_open { self.refresh_image(); }
            self.toggle_index(self.selected);
            self.shift_last_landed = Some((self.selected, true));
        }
    }

    fn toggle_index(&mut self, idx: usize) {
        if self.selection.contains(&idx) {
            self.selection.remove(&idx);
        } else {
            self.selection.insert(idx);
        }
    }

    // ── Slug/day boundary jumps ───────────────────────────────────────────────

    /// Group key for a file: `yyyy-MM-<slug>` when a slug is present, else `yyyy-MM-DD`.
    /// Matches MEX filename convention: slug files have no day component; day files have DD.
    fn group_key(f: &MediaFile) -> String {
        if !f.derived_slug.is_empty() {
            let month = if f.derived_date.len() >= 7 { &f.derived_date[..7] } else { &f.derived_date };
            format!("{}-{}", month, f.derived_slug)
        } else if f.derived_date.len() >= 10 {
            f.derived_date[..10].to_string()
        } else {
            f.derived_date.clone()
        }
    }

    /// Find the first index of the current group (scan backward from `pos`).
    fn group_start_of(&self, pos: usize) -> usize {
        let key = Self::group_key(&self.filtered[pos]);
        (0..=pos).rev()
            .take_while(|&i| Self::group_key(&self.filtered[i]) == key)
            .last()
            .unwrap_or(pos)
    }

    /// Find the last index of the current group (scan forward from `pos`).
    fn group_end_of(&self, pos: usize) -> usize {
        let n = self.filtered.len();
        let key = Self::group_key(&self.filtered[pos]);
        (pos..n)
            .take_while(|&i| Self::group_key(&self.filtered[i]) == key)
            .last()
            .unwrap_or(pos)
    }

    /// Toggle all indices in `range` in the selection:
    /// if every index is already selected → remove all; otherwise insert all.
    fn toggle_range(&mut self, lo: usize, hi: usize) {
        let all_selected = (lo..=hi).all(|i| self.selection.contains(&i));
        if all_selected {
            for i in lo..=hi { self.selection.remove(&i); }
        } else {
            for i in lo..=hi { self.selection.insert(i); }
        }
    }

    /// Home (non-selecting): jump to start of current slug/day group.
    /// If already at the group start, jump to start of the previous group.
    pub fn jump_home(&mut self) {
        if self.filtered.is_empty() { return; }
        let group_start = self.group_start_of(self.selected);
        if self.selected > group_start {
            self.selected = group_start;
        } else if group_start > 0 {
            let prev_end = group_start - 1;
            self.selected = self.group_start_of(prev_end);
        }
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
    }

    /// End (non-selecting): jump to the start of the next slug/day group.
    /// No-op if already in the last group.
    pub fn jump_end(&mut self) {
        if self.filtered.is_empty() { return; }
        let group_end = self.group_end_of(self.selected);
        let next_start = group_end + 1;
        if next_start < self.filtered.len() {
            self.selected = next_start;
        }
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
    }

    /// Shift-Home: toggle-range selection + cursor movement.
    ///
    /// - NOT at group start: toggles `group_start..=cursor`; cursor → group_start.
    /// - AT group start: toggles entire previous group; cursor → prev_group_start.
    pub fn jump_slug_day_prev(&mut self) {
        if self.filtered.is_empty() { return; }
        let group_start = self.group_start_of(self.selected);
        if self.selected > group_start {
            self.toggle_range(group_start, self.selected);
            self.selected = group_start;
        } else if group_start > 0 {
            let prev_end = group_start - 1;
            let prev_start = self.group_start_of(prev_end);
            self.toggle_range(prev_start, prev_end);
            self.selected = prev_start;
        }
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
    }

    /// Shift-End: toggle-range selection + cursor overshoot.
    ///
    /// Toggles `cursor..=group_end`; cursor moves to the **start of the next group**
    /// (overshooting the selection boundary). If at the last group, cursor moves to group_end.
    pub fn jump_slug_day_next(&mut self) {
        if self.filtered.is_empty() { return; }
        let group_end = self.group_end_of(self.selected);
        self.toggle_range(self.selected, group_end);
        let next_start = group_end + 1;
        self.selected = if next_start < self.filtered.len() { next_start } else { group_end };
        self.ensure_visible();
        if self.preview_open { self.refresh_image(); }
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

    /// Absolute path to the directory containing test images.
    /// Prefers the local `mex-media-root` if present; falls back to
    /// generating tiny synthetic images in a temp directory so tests
    /// always pass even without real media files checked out.
    fn test_media_root() -> String {
        for prefix in &["mex-media-root", "../mex-media-root"] {
            let p = PathBuf::from(prefix);
            if p.is_dir() {
                return p.canonicalize()
                    .unwrap_or(p)
                    .to_string_lossy()
                    .into_owned();
            }
        }
        create_synthetic_test_images()
    }

    fn create_synthetic_test_images() -> String {
        use image::{DynamicImage, RgbImage};
        let dir = std::env::temp_dir().join("mex_test_media_root");
        std::fs::create_dir_all(&dir).expect("create test media dir");
        for (name, fmt) in &[
            ("rolze.jpg", image::ImageFormat::Jpeg),
            ("bg.png", image::ImageFormat::Png),
        ] {
            let p = dir.join(name);
            if !p.exists() {
                let img = DynamicImage::ImageRgb8(RgbImage::new(8, 8));
                img.save_with_format(&p, *fmt).expect("write synthetic test image");
            }
        }
        dir.to_string_lossy().into_owned()
    }

    /// Build a test App where each entry in `image_names` becomes one
    /// MediaFile with that name as `target_path` (relative to test_media_root).
    fn make_test_app(image_names: &[&str]) -> App {        let (tx, _rx) = mpsc::channel();
        let image_state = ThreadProtocol::new(tx, None);
        let picker = Picker::halfblocks();
        let root = test_media_root();
        let files: Vec<crate::db::MediaFile> = image_names
            .iter()
            .enumerate()
            .map(|(i, name)| crate::db::MediaFile {
                id: i.to_string(),
                target_path: name.to_string(),
                derived_date: "2024-01-01".into(),
                ext: name.rsplit('.').next().unwrap_or("").into(),
                tags: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
            })
            .collect();
        App::new("test.db".into(), root, files, picker, image_state, "halfblocks".into())
    }

    /// Build a test App with extra non-image rows appended for navigation tests.
    #[allow(dead_code)]
    fn make_test_app_with_extra_rows(image_names: &[&str], extra: usize) -> App {
        let (tx, _rx) = mpsc::channel();
        let image_state = ThreadProtocol::new(tx, None);
        let picker = Picker::halfblocks();
        let root = test_media_root();
        let mut files: Vec<crate::db::MediaFile> = image_names
            .iter()
            .enumerate()
            .map(|(i, name)| crate::db::MediaFile {
                id: i.to_string(),
                target_path: name.to_string(),
                derived_date: "2024-01-01".into(),
                ext: name.rsplit('.').next().unwrap_or("").into(),
                tags: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
            })
            .collect();
        // Extra placeholder rows (non-existent paths — preview will clear gracefully).
        for i in 0..extra {
            files.push(crate::db::MediaFile {
                id: format!("extra-{i}"),
                target_path: format!("nonexistent-{i}.jpg"),
                derived_date: "2024-01-01".into(),
                ext: "jpg".into(),
                tags: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
            });
        }
        App::new("test.db".into(), root, files, picker, image_state, "halfblocks".into())
    }

    // ── Dispatch-count tests ────────────────────────────────────────────────

    /// First call to refresh_image should dispatch exactly one encode.
    #[test]
    fn first_open_dispatches_once() {
        let mut app = make_test_app(&["rolze.jpg"]);
        assert_eq!(app.encode_dispatch_count, 0);
        app.refresh_image();
        assert_eq!(app.encode_dispatch_count, 1, "first open must dispatch an encode");
    }

    /// Calling refresh_image again with the same path must NOT dispatch a
    /// second encode (the cached path short-circuits).
    #[test]
    fn same_path_no_second_dispatch() {
        let mut app = make_test_app(&["rolze.jpg"]);
        app.refresh_image();
        let count_after_first = app.encode_dispatch_count;
        app.refresh_image(); // same path, same selection
        assert_eq!(
            app.encode_dispatch_count, count_after_first,
            "second call with same path must NOT dispatch another encode"
        );
    }

    /// Closing and reopening the preview on the same item must not dispatch
    /// a new encode (ThreadProtocol is kept alive).
    #[test]
    fn close_reopen_no_redispatch() {
        let mut app = make_test_app(&["bg.png"]);
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

    /// Navigating to a different row and back must re-dispatch.
    #[test]
    fn navigate_away_and_back_dispatches() {
        let mut app = make_test_app(&["rolze.jpg", "bg.png", "rolze.jpg"]);
        app.toggle_preview();
        let after_first = app.encode_dispatch_count; // 1
        app.move_down(); // → bg.png
        let after_second = app.encode_dispatch_count; // 2
        assert!(after_second > after_first, "different image must dispatch");
        app.move_up(); // → rolze.jpg (different current_image_path than bg.png)
        let after_return = app.encode_dispatch_count; // 3
        assert!(after_return > after_second, "navigating back must dispatch");
    }

    /// DynamicImage is cached after first disk read; navigating away and back
    /// re-dispatches (new encode) but does not re-read from disk.
    #[test]
    fn dynimage_is_cached_after_first_load() {
        let mut app = make_test_app(&["rolze.jpg", "bg.png", "rolze.jpg"]);
        app.toggle_preview();
        assert_eq!(app.image_cache.len(), 1, "first open must populate DynamicImage cache");
        app.move_down();
        assert_eq!(app.image_cache.len(), 2);
        app.move_up();
        assert_eq!(app.image_cache.len(), 2, "cache must not evict prematurely");
    }

    // ── Timing tests ────────────────────────────────────────────────────────

    #[test]
    fn same_path_refresh_is_sub_millisecond() {
        let mut app = make_test_app(&["rolze.jpg"]);
        app.refresh_image();
        let t0 = Instant::now();
        app.refresh_image();
        let elapsed = t0.elapsed();
        assert!(
            elapsed.as_millis() < 1,
            "same-path refresh_image() must take <1 ms, took {}µs",
            elapsed.as_micros()
        );
    }

    #[test]
    fn cache_hit_faster_than_cold_read() {
        let mut app = make_test_app(&["rolze.jpg", "bg.png"]);

        let t_cold = Instant::now();
        app.refresh_image(); // cold read
        let cold_duration = t_cold.elapsed();

        app.move_down(); // navigate away
        app.move_up();   // back to rolze.jpg
        // Reset path to force cache-hit path (not same-path short-circuit).
        app.current_image_path = None;
        let t_hot = Instant::now();
        app.refresh_image(); // cache hit — no disk I/O
        let hot_duration = t_hot.elapsed();

        println!(
            "cold={cold_duration:?}  hot={hot_duration:?}  ratio={}",
            cold_duration.as_micros().max(1) / hot_duration.as_micros().max(1)
        );
        assert!(
            hot_duration <= cold_duration + std::time::Duration::from_millis(5),
            "DynamicImage cache-hit ({hot_duration:?}) must not be slower than cold read ({cold_duration:?})"
        );
    }

    // ── Path construction ───────────────────────────────────────────────────

    /// refresh_image must build the path as target_root / target_path and
    /// load the image when the file exists.
    #[test]
    fn refresh_image_uses_target_root_plus_target_path() {
        let mut app = make_test_app(&["rolze.jpg"]);
        app.refresh_image();
        let expected = PathBuf::from(test_media_root()).join("rolze.jpg");
        assert_eq!(
            app.current_image_path.as_ref(),
            Some(&expected),
            "current_image_path must equal target_root/target_path"
        );
    }

    /// Non-image extensions must NOT dispatch an encode.
    #[test]
    fn non_image_file_does_not_dispatch() {
        let mut app = make_test_app(&["rolze.jpg"]);
        // Override the file's ext to a non-image type to simulate an audio file.
        app.all_files[0].target_path = "some_audio.mp3".into();
        app.filtered[0].target_path = "some_audio.mp3".into();
        app.refresh_image();
        assert_eq!(app.encode_dispatch_count, 0, "non-image file must not dispatch encode");
        assert!(app.current_image_path.is_none());
    }

    // ── apply_filter / filter reset ─────────────────────────────────────────

    #[test]
    fn filter_clears_image_state() {
        let mut app = make_test_app(&["rolze.jpg"]);
        app.toggle_preview();
        assert!(app.current_image_path.is_some());
        app.push_filter_char('x');
        assert!(app.current_image_path.is_none(), "filter must clear current_image_path");
        assert!(!app.preview_open, "filter must close preview");
    }

    // ── UC-04 · Selecting Files ──────────────────────────────────────────────

    /// Build a test App with files arranged in named groups.
    /// Each entry in `groups` is `(slug_or_date, count)`.
    /// - If the group name looks like a date (`yyyy-MM-DD`), files use that as `derived_date`
    ///   with no slug → group key = date.
    /// - Otherwise the name is used as a `derived_slug` with a fixed month → group key = month-slug.
    fn make_grouped_app(groups: &[(&str, usize)]) -> App {
        let (tx, _rx) = mpsc::channel();
        let image_state = ThreadProtocol::new(tx, None);
        let picker = Picker::halfblocks();
        let mut files: Vec<crate::db::MediaFile> = Vec::new();
        let mut idx = 0usize;
        for (name, count) in groups {
            for _ in 0..*count {
                let (derived_date, derived_slug) = if name.len() == 10 && name.chars().nth(4) == Some('-') {
                    (name.to_string(), String::new())
                } else {
                    ("2024-01".to_string(), name.to_string())
                };
                files.push(crate::db::MediaFile {
                    id: idx.to_string(),
                    target_path: format!("nonexistent-{idx}.jpg"),
                    derived_date,
                    ext: "jpg".into(),
                    tags: vec![],
                    derived_slug,
                    caption_slug: String::new(),
                });
                idx += 1;
            }
        }
        App::new("test.db".into(), String::new(), files, picker, image_state, "halfblocks".into())
    }

    // ── Home (non-selecting) ────────────────────────────────────────────────

    #[test]
    fn home_jumps_to_group_start() {
        // Groups: A(3), B(3) — start at index 4 (middle of B)
        let mut app = make_grouped_app(&[("trip", 3), ("work", 3)]);
        app.selected = 4;
        app.jump_home();
        assert_eq!(app.selected, 3, "Home from middle of group B should land at B's start (index 3)");
        assert!(app.selection.is_empty(), "Home must not modify selection");
    }

    #[test]
    fn home_at_group_start_jumps_to_prev() {
        // Groups: A(3), B(3) — cursor at 3 (start of B)
        let mut app = make_grouped_app(&[("trip", 3), ("work", 3)]);
        app.selected = 3;
        app.jump_home();
        assert_eq!(app.selected, 0, "Home at start of B should jump to start of A (index 0)");
        assert!(app.selection.is_empty());
    }

    #[test]
    fn home_at_first_item_is_noop() {
        let mut app = make_grouped_app(&[("trip", 3)]);
        app.selected = 0;
        app.jump_home();
        assert_eq!(app.selected, 0, "Home at first item should be a no-op");
    }

    // ── End (non-selecting) ─────────────────────────────────────────────────

    #[test]
    fn end_jumps_to_start_of_next_group() {
        // Groups: A(3) indices 0-2, B(3) indices 3-5
        let mut app = make_grouped_app(&[("trip", 3), ("work", 3)]);
        app.selected = 1; // middle of A
        app.jump_end();
        assert_eq!(app.selected, 3, "End should jump to start of next group (index 3)");
        assert!(app.selection.is_empty(), "End must not modify selection");
    }

    #[test]
    fn end_at_last_group_is_noop() {
        let mut app = make_grouped_app(&[("trip", 3), ("work", 3)]);
        app.selected = 4; // inside last group
        app.jump_end();
        assert_eq!(app.selected, 4, "End at last group should be a no-op");
    }

    #[test]
    fn end_from_last_item_of_first_group_jumps_to_next() {
        // Groups: A(3) indices 0-2, B(3) indices 3-5
        let mut app = make_grouped_app(&[("trip", 3), ("work", 3)]);
        app.selected = 2; // last item of A
        app.jump_end();
        assert_eq!(app.selected, 3, "End from last item of A should jump to start of B");
    }

    // ── Shift-Home (toggle-range selection) ────────────────────────────────

    #[test]
    fn shift_home_selects_to_group_start() {
        // Groups: A(4) indices 0-3 — cursor at 2
        let mut app = make_grouped_app(&[("trip", 4)]);
        app.selected = 2;
        app.jump_slug_day_prev();
        assert_eq!(app.selected, 0, "cursor should move to group start");
        assert!(app.selection.contains(&0));
        assert!(app.selection.contains(&1));
        assert!(app.selection.contains(&2));
        assert_eq!(app.selection.len(), 3, "should select indices 0..=2");
    }

    #[test]
    fn shift_home_toggles_deselect() {
        // Groups: A(4) — cursor at 2, range already selected
        let mut app = make_grouped_app(&[("trip", 4)]);
        app.selected = 2;
        // Pre-select the range that Shift-Home would select
        app.selection.extend([0, 1, 2]);
        app.jump_slug_day_prev();
        assert!(app.selection.is_empty(), "Shift-Home on already-selected range should deselect");
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn shift_home_at_group_start_selects_prev_group() {
        // Groups: A(3) indices 0-2, B(3) indices 3-5 — cursor at 3 (start of B)
        let mut app = make_grouped_app(&[("trip", 3), ("work", 3)]);
        app.selected = 3;
        app.jump_slug_day_prev();
        assert_eq!(app.selected, 0, "cursor should jump to start of A");
        assert!(app.selection.contains(&0));
        assert!(app.selection.contains(&1));
        assert!(app.selection.contains(&2));
        assert_eq!(app.selection.len(), 3, "should select entire previous group (A)");
        assert!(!app.selection.contains(&3), "current position (start of B) must not be selected");
    }

    #[test]
    fn shift_home_at_first_group_start_is_noop() {
        let mut app = make_grouped_app(&[("trip", 3)]);
        app.selected = 0;
        app.jump_slug_day_prev();
        assert_eq!(app.selected, 0);
        assert!(app.selection.is_empty());
    }

    // ── Shift-End (toggle-range + cursor overshoot) ─────────────────────────

    #[test]
    fn shift_end_selects_to_group_end() {
        // Groups: A(4) indices 0-3, B(3) indices 4-6 — cursor at 1
        let mut app = make_grouped_app(&[("trip", 4), ("work", 3)]);
        app.selected = 1;
        app.jump_slug_day_next();
        assert!(app.selection.contains(&1));
        assert!(app.selection.contains(&2));
        assert!(app.selection.contains(&3));
        assert_eq!(app.selection.len(), 3, "should select 1..=3 (rest of group A)");
    }

    #[test]
    fn shift_end_cursor_overshoots_to_next_group_start() {
        // Groups: A(4) indices 0-3, B(3) indices 4-6 — cursor at 1
        let mut app = make_grouped_app(&[("trip", 4), ("work", 3)]);
        app.selected = 1;
        app.jump_slug_day_next();
        assert_eq!(app.selected, 4, "cursor should overshoot to start of next group (index 4)");
        assert!(!app.selection.contains(&4), "start of next group must NOT be in selection");
    }

    #[test]
    fn shift_end_toggles_deselect() {
        // Groups: A(4) — cursor at 1, range already selected
        let mut app = make_grouped_app(&[("trip", 4), ("work", 3)]);
        app.selected = 1;
        app.selection.extend([1, 2, 3]); // pre-select what Shift-End would select
        app.jump_slug_day_next();
        assert!(
            !app.selection.contains(&1) && !app.selection.contains(&2) && !app.selection.contains(&3),
            "Shift-End on already-selected range should deselect"
        );
    }

    #[test]
    fn shift_end_at_last_group_selects_rest_no_cursor_move() {
        // Groups: A(3) indices 0-2 only — cursor at 1
        let mut app = make_grouped_app(&[("trip", 3)]);
        app.selected = 1;
        app.jump_slug_day_next();
        assert!(app.selection.contains(&1));
        assert!(app.selection.contains(&2));
        assert_eq!(app.selection.len(), 2);
        // No next group — cursor stays at last item of group
        assert_eq!(app.selected, 2, "cursor should stay at last item when no next group exists");
    }

    // ── Shift-Up/Down (toggle both, skip re-toggle when continuing) ─────────

    #[test]
    fn shift_down_first_press_toggles_start_and_landed() {
        let mut app = make_grouped_app(&[("trip", 5)]);
        app.selected = 0;
        app.extend_selection_down(); // toggle 0 (sel), move→1, toggle 1 (sel)
        assert!(app.selection.contains(&0));
        assert!(app.selection.contains(&1));
        assert_eq!(app.selection.len(), 2);
    }

    #[test]
    fn shift_down_continuing_only_toggles_landed() {
        let mut app = make_grouped_app(&[("trip", 5)]);
        app.selected = 0;
        app.extend_selection_down(); // toggle 0, move→1, toggle 1 → {0,1}
        app.extend_selection_down(); // continuing: skip 1, move→2, toggle 2 → {0,1,2}
        assert!(app.selection.contains(&0));
        assert!(app.selection.contains(&1), "item 1 must NOT be double-toggled");
        assert!(app.selection.contains(&2));
        assert_eq!(app.selection.len(), 3);
    }

    #[test]
    fn shift_down_continuing_builds_range() {
        let mut app = make_grouped_app(&[("trip", 6)]);
        app.selected = 0;
        app.extend_selection_down();
        app.extend_selection_down();
        app.extend_selection_down();
        app.extend_selection_down(); // → {0,1,2,3,4}
        assert_eq!(app.selection.len(), 5);
        for i in 0..=4 { assert!(app.selection.contains(&i)); }
    }

    #[test]
    fn shift_direction_change_toggles_current_on_reverse() {
        let mut app = make_grouped_app(&[("trip", 5)]);
        app.selected = 0;
        app.extend_selection_down(); // {0,1}, cursor=1
        app.extend_selection_down(); // {0,1,2}, cursor=2
        // Reverse: not continuing down from 2, so toggle 2 (desel) + move + toggle 1 (desel)
        app.extend_selection_up();   // {0}, cursor=1
        assert!(app.selection.contains(&0));
        assert!(!app.selection.contains(&1));
        assert!(!app.selection.contains(&2));
        assert_eq!(app.selection.len(), 1);
    }

    #[test]
    fn shift_up_continuing_builds_range() {
        let mut app = make_grouped_app(&[("trip", 5)]);
        app.selected = 4;
        app.extend_selection_up();
        app.extend_selection_up();
        app.extend_selection_up(); // → {4,3,2,1}
        assert_eq!(app.selection.len(), 4);
        for i in 1..=4 { assert!(app.selection.contains(&i)); }
    }

    #[test]
    fn shift_normal_nav_resets_continuation() {
        let mut app = make_grouped_app(&[("trip", 5)]);
        app.selected = 0;
        app.extend_selection_down(); // {0,1}, state=(1,down)
        app.move_down();             // normal nav, resets shift state, cursor=2
        app.extend_selection_down(); // fresh start: toggle 2, move→3, toggle 3 → {0,1,2,3}
        assert!(app.selection.contains(&2), "fresh start toggles current");
        assert!(app.selection.contains(&3));
    }

    #[test]
    fn shift_down_preserves_existing_selection() {
        let mut app = make_grouped_app(&[("trip", 5)]);
        app.selection.insert(4);
        app.selected = 1;
        app.extend_selection_down(); // toggles 1 and 2; item 4 untouched
        assert!(app.selection.contains(&4));
        assert!(app.selection.contains(&1));
        assert!(app.selection.contains(&2));
    }

    // ── Space toggle ─────────────────────────────────────────────────────────

    #[test]
    fn space_toggle_independent_of_anchor() {
        let mut app = make_grouped_app(&[("trip", 5)]);
        app.selected = 2;
        app.toggle_selection(); // select index 2
        assert!(app.selection.contains(&2));
        app.toggle_selection(); // deselect index 2
        assert!(!app.selection.contains(&2));
    }

    // ── Esc clears selection ─────────────────────────────────────────────────

    #[test]
    fn selection_cleared_before_preview_on_esc_order() {
        let mut app = make_grouped_app(&[("trip", 3)]);
        app.selection.insert(0);
        app.selection.insert(1);
        app.preview_open = true;
        // Simulate first Esc: clears selection (preview stays open)
        app.clear_selection();
        assert!(app.selection.is_empty(), "selection should be cleared");
        assert!(app.preview_open, "preview should still be open after first Esc-equivalent");
        // Simulate second Esc: closes preview
        app.preview_open = false;
        assert!(!app.preview_open);
    }
}
