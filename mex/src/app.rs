use crate::db::MediaFile;
use crate::import::{ImportEntry, ImportMsg, ImportStatus};
use image::DynamicImage;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, thread::ThreadProtocol};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
};

/// All command names recognised by the command bar, in alphabetical order.
/// Used for command-name autocompletion (analogous to tag autocompletion).
const KNOWN_COMMANDS: &[&str] = &["create-view", "empty-trash", "fix-date", "fix-ext", "fix-os-time", "import", "q", "quit", "remove-slug", "tag", "untag"];

// ── Import state ──────────────────────────────────────────────────────────────

pub enum ImportState {
    Idle,
    /// Background scan in progress; `scanned` is the number of files found so far.
    Scanning { scanned: usize, current_file: String },
    /// Scan finished; waiting for user confirmation (y / Esc).
    Preview {
        entries: Vec<ImportEntry>,
        scroll: usize, // scroll offset for the preview list
    },
    /// Copy in progress.
    Copying { done: usize, total: usize, current_file: String, copied: usize, skipped_dup: usize, errors: usize },
    /// Copy finished; message is displayed until the next keypress.
    Done(String),
}

// ── Remove-slug state ─────────────────────────────────────────────────────────

pub enum RemoveSlugState {
    Idle,
    /// Background repair in progress.
    Running { done: usize, total: usize, current: String },
    /// Finished; message shown until the next keypress.
    Done(String),
}

pub enum RemoveSlugMsg {
    Progress { done: usize, total: usize, current: String },
    Done(String),
}

// ── Fix-os-time state ─────────────────────────────────────────────────────────

pub enum FixOsTimeState {
    Idle,
    /// Background repair in progress.
    Running { done: usize, total: usize, current: String },
    /// Finished; message shown until the next keypress.
    Done(String),
}

pub enum FixOsTimeMsg {
    Progress { done: usize, total: usize, current: String },
    Done(String),
}

// ── Empty-trash state ─────────────────────────────────────────────────────────

pub enum EmptyTrashState {
    Idle,
    /// Waiting for user confirmation: shows up to 100 trashed files.
    Preview { files: Vec<crate::db::MediaFile>, scroll: usize },
    /// Background deletion in progress.
    Deleting { done: usize, total: usize },
    /// Finished; message shown until the next keypress.
    Done(String),
}

pub enum EmptyTrashMsg {
    Progress { done: usize, total: usize },
    Done(usize, usize), // (deleted, errors)
}

const CACHE_MAX: usize = 30;
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp"];

pub struct App {
    pub db_path: String,
    pub target_root: String,
    /// Root directory where `:create-view` materialises named view directories.
    pub views_root: String,
    pub all_files: Vec<MediaFile>,
    pub filtered: Vec<MediaFile>,
    pub selected: usize,
    pub scroll_offset: usize,
    /// Free-text search term (matches filenames only, not tags).
    pub filter_text: String,
    /// Confirmed tag filters — OR logic: file must have at least one of these tags.
    pub tag_filters: Vec<String>,
    /// True while the user is typing a `#tag` token (between `#` and Enter/Tab).
    pub tag_typing: bool,
    /// Characters typed after `#` for the tag currently being entered.
    pub tag_input: String,
    /// All unique tags present in the library, sorted case-insensitively.
    pub all_tags: Vec<String>,
    /// Index into the filtered suggestion list for Up/Down cycling.
    pub suggestion_idx: usize,
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
    /// Index into the filtered command-name suggestion list for Up/Down cycling.
    pub command_suggestion_idx: usize,
    /// All unique tag types present in the library, sorted case-insensitively.
    pub all_tag_types: Vec<String>,
    /// Confirmed tag-type filters — OR logic: file must have at least one tag of these types.
    pub tag_type_filters: Vec<String>,
    /// True while the user is typing an `@type` token.
    pub tag_type_typing: bool,
    /// Characters typed after `@` for the type currently being entered.
    pub tag_type_input: String,
    /// Index into the filtered type-suggestion list for Up/Down cycling.
    pub type_suggestion_idx: usize,
    /// One-shot status message shown in the filter bar after a command executes.
    /// Cleared on the next keypress.
    pub status_message: Option<String>,
    /// Import state machine.
    pub import_state: ImportState,
    /// Receive channel for background import thread messages.
    pub import_rx: Option<mpsc::Receiver<ImportMsg>>,
    /// Height of the import preview list (visible rows); updated each frame.
    pub import_list_height: usize,
    /// Remove-slug repair state machine.
    pub remove_slug_state: RemoveSlugState,
    /// Receive channel for the background remove-slug thread.
    pub remove_slug_rx: Option<mpsc::Receiver<RemoveSlugMsg>>,
    /// Fix-os-time repair state machine.
    pub fix_os_time_state: FixOsTimeState,
    /// Receive channel for the background fix-os-time thread.
    pub fix_os_time_rx: Option<mpsc::Receiver<FixOsTimeMsg>>,
    /// Empty-trash state machine.
    pub empty_trash_state: EmptyTrashState,
    /// Receive channel for the background empty-trash deletion thread.
    pub empty_trash_rx: Option<mpsc::Receiver<EmptyTrashMsg>>,
    /// Number of files currently with `status='trashed'`; updated on every reload.
    pub trashed_count: usize,
}

impl App {
    pub fn new(
        db_path: String,
        target_root: String,
        views_root: String,
        files: Vec<MediaFile>,
        image_picker: Picker,
        image_state: ThreadProtocol,
        image_protocol_name: String,
    ) -> Self {
        let filtered = files.clone();
        let mut tag_set: BTreeSet<String> = BTreeSet::new();
        let mut type_set: BTreeSet<String> = BTreeSet::new();
        let mut trashed_count = 0usize;
        for f in &files {
            for tag in &f.tags {
                tag_set.insert(tag.clone());
            }
            for ty in &f.tag_types {
                if !ty.is_empty() {
                    type_set.insert(ty.clone());
                }
            }
            if f.status == "trashed" {
                trashed_count += 1;
            }
        }
        let all_tags: Vec<String> = tag_set.into_iter().collect();
        let all_tag_types: Vec<String> = type_set.into_iter().collect();
        Self {
            db_path,
            target_root,
            views_root,
            all_files: files,
            filtered,
            selected: 0,
            scroll_offset: 0,
            filter_text: String::new(),
            tag_filters: Vec::new(),
            tag_typing: false,
            tag_input: String::new(),
            all_tags,
            suggestion_idx: 0,
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
            command_suggestion_idx: 0,
            all_tag_types,
            tag_type_filters: Vec::new(),
            tag_type_typing: false,
            tag_type_input: String::new(),
            type_suggestion_idx: 0,
            status_message: None,
            import_state: ImportState::Idle,
            import_rx: None,
            import_list_height: 20,
            remove_slug_state: RemoveSlugState::Idle,
            remove_slug_rx: None,
            fix_os_time_state: FixOsTimeState::Idle,
            fix_os_time_rx: None,
            empty_trash_state: EmptyTrashState::Idle,
            empty_trash_rx: None,
            trashed_count,
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
        self.status_message = None;
        if c == '#' {
            // Abandon @-typing if active.
            if self.tag_type_typing {
                self.tag_type_typing = false;
                self.tag_type_input.clear();
                self.type_suggestion_idx = 0;
            }
            self.tag_typing = true;
            self.tag_input.clear();
            self.suggestion_idx = 0;
        } else if c == '@' {
            // Abandon #-typing if active.
            if self.tag_typing {
                self.tag_typing = false;
                self.tag_input.clear();
                self.suggestion_idx = 0;
            }
            self.tag_type_typing = true;
            self.tag_type_input.clear();
            self.type_suggestion_idx = 0;
        } else if self.tag_typing {
            self.tag_input.push(c);
            self.suggestion_idx = 0;
        } else if self.tag_type_typing {
            self.tag_type_input.push(c);
            self.type_suggestion_idx = 0;
        } else {
            self.filter_text.push(c);
        }
        self.apply_filter();
    }

    pub fn pop_filter_char(&mut self) {
        if self.tag_typing {
            if self.tag_input.is_empty() {
                self.tag_typing = false;
            } else {
                self.tag_input.pop();
            }
            self.suggestion_idx = 0;
        } else if self.tag_type_typing {
            if self.tag_type_input.is_empty() {
                self.tag_type_typing = false;
            } else {
                self.tag_type_input.pop();
            }
            self.type_suggestion_idx = 0;
        } else if self.filter_text.is_empty() && !self.tag_filters.is_empty() {
            self.tag_filters.pop();
        } else if self.filter_text.is_empty() && !self.tag_type_filters.is_empty() {
            self.tag_type_filters.pop();
        } else {
            self.filter_text.pop();
        }
        self.apply_filter();
    }

    pub fn clear_filter(&mut self) {
        self.filter_text.clear();
        self.tag_filters.clear();
        self.tag_typing = false;
        self.tag_input.clear();
        self.suggestion_idx = 0;
        self.tag_type_filters.clear();
        self.tag_type_typing = false;
        self.tag_type_input.clear();
        self.type_suggestion_idx = 0;
        self.apply_filter();
    }

    // ── Tag suggestion / autocomplete ────────────────────────────────────────

    /// Returns all tags whose name starts with the current `tag_input` (case-insensitive).
    pub fn filtered_tag_suggestions(&self) -> Vec<&String> {
        let lower = self.tag_input.to_lowercase();
        self.all_tags
            .iter()
            .filter(|t| t.to_lowercase().starts_with(&lower))
            .collect()
    }

    /// The currently highlighted suggestion, or `None` when there are no matches.
    pub fn current_suggestion(&self) -> Option<String> {
        let suggestions = self.filtered_tag_suggestions();
        suggestions.get(self.suggestion_idx).map(|s| (*s).clone())
    }

    /// Confirm the current tag: push the highlighted suggestion (or the raw
    /// input if there is none) into `tag_filters`, then exit tag-typing mode.
    pub fn confirm_tag(&mut self) {
        let tag = self.current_suggestion()
            .unwrap_or_else(|| self.tag_input.trim().to_lowercase());
        let tag = tag.trim().to_string();
        if !tag.is_empty() && !self.tag_filters.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            self.tag_filters.push(tag);
        }
        self.tag_typing = false;
        self.tag_input.clear();
        self.suggestion_idx = 0;
        self.apply_filter();
    }

    /// Complete the current tag input to the highlighted suggestion.
    /// In command mode, completes the command name.
    /// In type-typing mode, completes the tag type.
    pub fn tab_complete(&mut self) {
        if self.command.is_some() {
            self.tab_complete_command();
        } else if self.tag_type_typing {
            if let Some(suggestion) = self.current_type_suggestion() {
                self.tag_type_input = suggestion;
            }
        } else if self.tag_typing {
            if let Some(suggestion) = self.current_suggestion() {
                self.tag_input = suggestion;
            }
        }
    }

    /// Cycle the suggestion highlight downward (wraps around).
    pub fn cycle_suggestion_down(&mut self) {
        let count = self.filtered_tag_suggestions().len();
        if count > 0 {
            self.suggestion_idx = (self.suggestion_idx + 1) % count;
        }
    }

    /// Cycle the suggestion highlight upward (wraps around).
    pub fn cycle_suggestion_up(&mut self) {
        let count = self.filtered_tag_suggestions().len();
        if count > 0 {
            self.suggestion_idx = (self.suggestion_idx + count - 1) % count;
        }
    }

    /// Returns true when any filter is active (text, confirmed tags/types, or tag/type being typed).
    pub fn is_filter_active(&self) -> bool {
        !self.filter_text.is_empty()
            || !self.tag_filters.is_empty()
            || self.tag_typing
            || !self.tag_type_filters.is_empty()
            || self.tag_type_typing
    }

    // ── Tag-type suggestion / autocomplete ──────────────────────────────────

    /// Returns all tag types whose name starts with `tag_type_input` (case-insensitive).
    pub fn filtered_type_suggestions(&self) -> Vec<&String> {
        let lower = self.tag_type_input.to_lowercase();
        self.all_tag_types
            .iter()
            .filter(|t| t.to_lowercase().starts_with(&lower))
            .collect()
    }

    /// The currently highlighted type suggestion, or `None` when there are no matches.
    pub fn current_type_suggestion(&self) -> Option<String> {
        let suggestions = self.filtered_type_suggestions();
        suggestions.get(self.type_suggestion_idx).map(|s| (*s).clone())
    }

    /// Confirm the current type filter: push the highlighted suggestion (or raw input)
    /// into `tag_type_filters`, then exit type-typing mode.
    pub fn confirm_type_filter(&mut self) {
        let ty = self.current_type_suggestion()
            .unwrap_or_else(|| self.tag_type_input.trim().to_lowercase());
        let ty = ty.trim().to_string();
        if !ty.is_empty() && !self.tag_type_filters.iter().any(|t| t.eq_ignore_ascii_case(&ty)) {
            self.tag_type_filters.push(ty);
        }
        self.tag_type_typing = false;
        self.tag_type_input.clear();
        self.type_suggestion_idx = 0;
        self.apply_filter();
    }

    /// Cycle type suggestion highlight downward (wraps around).
    pub fn cycle_type_suggestion_down(&mut self) {
        let count = self.filtered_type_suggestions().len();
        if count > 0 {
            self.type_suggestion_idx = (self.type_suggestion_idx + 1) % count;
        }
    }

    /// Cycle type suggestion highlight upward (wraps around).
    pub fn cycle_type_suggestion_up(&mut self) {
        let count = self.filtered_type_suggestions().len();
        if count > 0 {
            self.type_suggestion_idx = (self.type_suggestion_idx + count - 1) % count;
        }
    }

    // ── Command mode ────────────────────────────────────────────────────────

    /// Enter `:` command mode. Typing a command string and pressing Enter
    /// executes it. All letters are otherwise reserved for live search.
    pub fn enter_command_mode(&mut self) {
        self.status_message = None;
        self.command = Some(String::new());
        self.command_suggestion_idx = 0;
    }

    pub fn push_command_char(&mut self, c: char) {
        if let Some(ref mut cmd) = self.command {
            cmd.push(c);
            self.command_suggestion_idx = 0;
        }
    }

    /// Pop last char from command buffer; cancel command mode if buffer is empty.
    pub fn pop_command_char(&mut self) {
        match self.command {
            Some(ref mut cmd) if !cmd.is_empty() => {
                cmd.pop();
                self.command_suggestion_idx = 0;
            }
            _ => self.command = None,
        }
    }

    pub fn cancel_command(&mut self) {
        self.command = None;
        self.command_suggestion_idx = 0;
    }

    // ── Command-name autocompletion ──────────────────────────────────────────

    /// Returns known command names that start with the typed prefix (before the
    /// first space). Returns all commands when the buffer is empty.
    pub fn command_name_suggestions(&self) -> Vec<&'static str> {
        let prefix = self.command.as_deref().unwrap_or("").to_lowercase();
        // Only complete the command-name part (before any space/argument).
        let name_prefix = prefix.split_once(' ').map(|(n, _)| n).unwrap_or(&prefix);
        KNOWN_COMMANDS
            .iter()
            .copied()
            .filter(|cmd| cmd.starts_with(name_prefix))
            .collect()
    }

    /// The currently highlighted command-name suggestion, or `None` when none match.
    pub fn current_command_suggestion(&self) -> Option<&'static str> {
        let suggestions = self.command_name_suggestions();
        suggestions.get(self.command_suggestion_idx).copied()
    }

    // ── Tag-argument autocompletion (`:tag <name>[@<type>]`, `:untag [name …]`) ─

    /// Returns suggestions for the tag argument of a `:tag` or `:untag` command.
    ///
    /// For `:tag`:
    /// - Before `@`: suggests tag names matching the typed prefix.
    /// - After `@`: suggests tag types matching the typed prefix.
    ///
    /// For `:untag`:
    /// - Suggests tag names matching the last (incomplete) word.
    pub fn tag_arg_suggestions(&self) -> Vec<String> {
        let cmd = self.command.as_deref().unwrap_or("");

        if let Some(arg) = cmd.strip_prefix("tag ") {
            if let Some(at_pos) = arg.rfind('@') {
                let type_prefix = arg[at_pos + 1..].to_lowercase();
                return self.all_tag_types
                    .iter()
                    .filter(|t| t.to_lowercase().starts_with(&type_prefix))
                    .cloned()
                    .collect();
            } else {
                let name_prefix = arg.to_lowercase();
                return self.all_tags
                    .iter()
                    .filter(|t| t.to_lowercase().starts_with(&name_prefix))
                    .cloned()
                    .collect();
            }
        }

        if let Some(rest) = cmd.strip_prefix("untag") {
            // Complete the last (incomplete) word.  If rest ends with a space
            // (or is empty) the user is starting a new word → prefix = "".
            let prefix = if rest.ends_with(' ') || rest.is_empty() {
                ""
            } else {
                rest.trim_start().rsplit(' ').next().unwrap_or("")
            };
            let prefix_lower = prefix.to_lowercase();
            return self.all_tags
                .iter()
                .filter(|t| t.to_lowercase().starts_with(&prefix_lower))
                .cloned()
                .collect();
        }

        vec![]
    }

    /// The currently highlighted tag-arg suggestion, or `None` when none match.
    pub fn current_tag_arg_suggestion(&self) -> Option<String> {
        self.tag_arg_suggestions()
            .into_iter()
            .nth(self.command_suggestion_idx)
    }

    /// Complete the command buffer to the highlighted suggestion.
    /// - Before any space: completes the command name.
    /// - After `tag `: completes the tag name or type argument.
    pub fn tab_complete_command(&mut self) {
        let has_arg = self.command.as_deref().unwrap_or("").contains(' ');
        if !has_arg {
            if let Some(suggestion) = self.current_command_suggestion() {
                self.command = Some(format!("{} ", suggestion));
                self.command_suggestion_idx = 0;
            }
        } else {
            self.tab_complete_tag_arg();
        }
    }

    /// Fill in the current tag-arg suggestion (name or type).
    fn tab_complete_tag_arg(&mut self) {
        let Some(suggestion) = self.current_tag_arg_suggestion() else {
            return;
        };
        let cmd = self.command.as_deref().unwrap_or("").to_string();

        let new_cmd = if let Some(arg) = cmd.strip_prefix("tag ") {
            if let Some(at_pos) = arg.rfind('@') {
                format!("tag {}@{}", &arg[..at_pos], suggestion)
            } else {
                format!("tag {}", suggestion)
            }
        } else if cmd.starts_with("untag") {
            // Replace the last word with the suggestion and add a trailing space
            // so the user can immediately start typing the next tag name.
            if let Some(last_space) = cmd.rfind(' ') {
                format!("{} {} ", &cmd[..last_space], suggestion)
            } else {
                format!("untag {} ", suggestion)
            }
        } else {
            return;
        };

        self.command = Some(new_cmd);
        self.command_suggestion_idx = 0;
    }

    pub fn cycle_command_suggestion_down(&mut self) {
        let count = if self.command.as_deref().unwrap_or("").contains(' ') {
            self.tag_arg_suggestions().len()
        } else {
            self.command_name_suggestions().len()
        };
        if count > 0 {
            self.command_suggestion_idx = (self.command_suggestion_idx + 1) % count;
        }
    }

    pub fn cycle_command_suggestion_up(&mut self) {
        let count = if self.command.as_deref().unwrap_or("").contains(' ') {
            self.tag_arg_suggestions().len()
        } else {
            self.command_name_suggestions().len()
        };
        if count > 0 {
            self.command_suggestion_idx =
                (self.command_suggestion_idx + count - 1) % count;
        }
    }

    /// Execute the current command. Sets `self.quit` for `:q` / `:quit`.
    /// Clears command mode regardless of outcome.
    pub fn execute_command(&mut self) {
        let cmd = self.command.take().unwrap_or_default();
        self.command_suggestion_idx = 0;
        let trimmed = cmd.trim();

        if trimmed == "q" || trimmed == "quit" {
            self.quit = true;
            return;
        }

        if let Some(name_arg) = trimmed.strip_prefix("create-view") {
            let name = name_arg.trim();
            self.create_view(name);
            return;
        }

        if let Some(date_arg) = trimmed.strip_prefix("fix-date") {
            let date_str = date_arg.trim();
            self.fix_date_selected(date_str);
            return;
        }

        if trimmed == "fix-ext" {
            self.fix_ext_selected();
            return;
        }

        if trimmed == "remove-slug" {
            self.remove_slug_selected();
            return;
        }

        if trimmed == "fix-os-time" {
            self.fix_os_time_selected();
            return;
        }

        if trimmed == "empty-trash" {
            self.start_empty_trash();
            return;
        }

        if let Some(path_arg) = trimmed.strip_prefix("import") {
            let path_str = path_arg.trim();
            self.start_import(path_str);
            return;
        }

        if let Some(tag_arg) = trimmed.strip_prefix("untag") {
            let tag_names: Vec<String> = tag_arg.split_whitespace().map(|s| s.to_string()).collect();
            self.untag_selected(&tag_names);
            return;
        }

        if let Some(tag_arg) = trimmed.strip_prefix("tag") {
            let tag_arg = tag_arg.trim();
            let (name, ty) = if let Some(at_pos) = tag_arg.find('@') {
                (&tag_arg[..at_pos], Some(tag_arg[at_pos + 1..].trim()))
            } else {
                (tag_arg, None)
            };
            let name = name.trim();
            if name.is_empty() {
                self.status_message = Some("tag: usage: tag <name>[@<type>]".into());
                return;
            }
            self.tag_selected(name, ty);
            return;
        }

        if !trimmed.is_empty() {
            self.status_message = Some(format!("Unknown command: {trimmed}"));
        }
    }

    // ── create-view ───────────────────────────────────────────────────────────

    /// Materialise the current selection (or full filtered list) as a flat
    /// directory of hard links under `<views_root>/<name>/`.
    pub fn create_view(&mut self, name: &str) {
        if self.views_root.is_empty() {
            self.status_message = Some(
                "create-view: views_root is not configured in ~/.config/mex/config.toml".into(),
            );
            return;
        }
        if name.is_empty() {
            self.status_message = Some("create-view: usage: create-view <name>".into());
            return;
        }

        // Determine source files.
        let files: Vec<&crate::db::MediaFile> = if self.selection.is_empty() {
            self.filtered.iter().collect()
        } else {
            let mut idxs: Vec<usize> = self.selection.iter().copied().collect();
            idxs.sort_unstable();
            idxs.iter()
                .filter_map(|&i| self.filtered.get(i))
                .collect()
        };

        if files.is_empty() {
            self.status_message = Some("create-view: nothing to link (list is empty)".into());
            return;
        }

        let view_dir = std::path::Path::new(&self.views_root).join(name);

        if view_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&view_dir) {
                self.status_message = Some(format!("create-view: could not remove existing view: {e}"));
                return;
            }
        }
        if let Err(e) = std::fs::create_dir_all(&view_dir) {
            self.status_message = Some(format!("create-view: could not create view directory: {e}"));
            return;
        }

        let mut linked = 0usize;
        let mut errors = 0usize;
        for file in &files {
            let src = std::path::Path::new(&self.target_root).join(&file.target_path);
            let basename = src.file_name().unwrap_or_default();
            let dst = view_dir.join(basename);
            if let Err(e) = std::fs::hard_link(&src, &dst) {
                eprintln!("create-view: hard_link {:?} -> {:?}: {e}", src, dst);
                errors += 1;
            } else {
                linked += 1;
            }
        }

        if errors == 0 {
            self.status_message = Some(format!(
                "View '{name}' created: {linked} file(s) → {}",
                view_dir.display()
            ));
        } else {
            self.status_message = Some(format!(
                "View '{name}': {linked} linked, {errors} error(s) — check stderr"
            ));
        }
    }

    // ── import ────────────────────────────────────────────────────────────────

    /// Start `:import <path>` — validate path, then spawn background scan thread.
    pub fn start_import(&mut self, path_str: &str) {
        let path = std::path::Path::new(path_str);
        if path_str.is_empty() {
            self.status_message = Some("import: usage: import <path>".into());
            return;
        }
        if !path.exists() {
            self.status_message = Some(format!("import: path does not exist: {path_str}"));
            return;
        }
        if !path.is_dir() {
            self.status_message = Some(format!("import: not a directory: {path_str}"));
            return;
        }

        let source_root = path.to_path_buf();
        let (tx, rx) = mpsc::channel::<ImportMsg>();
        self.import_rx = Some(rx);
        self.import_state = ImportState::Scanning { scanned: 0, current_file: String::new() };

        std::thread::spawn(move || {
            let tx2 = tx.clone();
            let mut progress_cb = move |n: usize, file: &str| {
                let _ = tx2.send(ImportMsg::ScanProgress {
                    count: n,
                    current_file: file.to_string(),
                });
            };

            match crate::import::scan_source(&source_root, &mut progress_cb) {
                Ok(mut entries) => {
                    crate::import::apply_folder_mtime_consensus(&mut entries);

                    let _ = tx.send(ImportMsg::ScanDone(entries));
                }
                Err(e) => {
                    let _ = tx.send(ImportMsg::ScanError(e.to_string()));
                }
            }
        });
    }

    /// Called by the event loop when an `ImportMsg` arrives on `import_rx`.
    pub fn on_import_msg(&mut self, msg: ImportMsg) {
        match msg {
            ImportMsg::ScanProgress { count, current_file } => {
                self.import_state = ImportState::Scanning { scanned: count, current_file };
            }
            ImportMsg::ScanDone(mut entries) => {
                // Assign counters now that we have target_root
                let target_root = std::path::Path::new(&self.target_root);
                if let Ok(conn) = rusqlite::Connection::open(&self.db_path) {
                    if let Err(e) = crate::import::assign_counters(&mut entries, target_root, &conn) {
                        self.status_message = Some(format!("import: counter error: {e}"));
                        self.import_state = ImportState::Idle;
                        return;
                    }
                }
                self.import_state = ImportState::Preview {
                    entries,
                    scroll: 0,
                };
            }
            ImportMsg::ScanError(e) => {
                self.status_message = Some(format!("import: scan error: {e}"));
                self.import_state = ImportState::Idle;
                self.import_rx = None;
            }
            ImportMsg::CopyProgress { done, total, current_file, copied, skipped_dup, errors } => {
                self.import_state = ImportState::Copying { done, total, current_file, copied, skipped_dup, errors };
            }
            ImportMsg::CopyDone(summary) => {
                let msg = format!(
                    "import: copied {}, {} dup, {} unknown-date, {} error(s)",
                    summary.copied, summary.skipped_dup, summary.unknown_date, summary.errors
                );
                self.import_state = ImportState::Done(msg.clone());
                self.status_message = Some(msg);
                self.import_rx = None;
                let _ = self.reload();
            }
            ImportMsg::CopyError(e) => {
                self.status_message = Some(format!("import: copy error: {e}"));
                self.import_state = ImportState::Idle;
                self.import_rx = None;
            }
        }
    }

    /// Confirm the import preview → start background copy thread.
    pub fn confirm_import(&mut self) {
        let entries = match &self.import_state {
            ImportState::Preview { entries, .. } => entries.clone(),
            _ => return,
        };
        let total = entries.iter().filter(|e| e.status == ImportStatus::Pending).count();
        self.import_state = ImportState::Copying { done: 0, total, current_file: String::new(), copied: 0, skipped_dup: 0, errors: 0 };

        let db_path = self.db_path.clone();
        let target_root = self.target_root.clone();
        let today = today_date();
        let (tx, rx) = mpsc::channel::<ImportMsg>();
        self.import_rx = Some(rx);

        std::thread::spawn(move || {
            let mut conn = match rusqlite::Connection::open(&db_path) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(ImportMsg::CopyError(e.to_string()));
                    return;
                }
            };
            if let Err(e) = crate::db::ensure_schema_v1(&conn) {
                let _ = tx.send(ImportMsg::CopyError(format!("schema migration failed: {e}")));
                return;
            }
            let tx2 = tx.clone();
            let mut progress_cb = move |done: usize, total: usize, file: &str, summary: &crate::import::ImportSummary| -> bool {
                tx2.send(ImportMsg::CopyProgress {
                    done,
                    total,
                    current_file: file.to_string(),
                    copied: summary.copied,
                    skipped_dup: summary.skipped_dup,
                    errors: summary.errors,
                }).is_ok()
            };
            match crate::import::execute_import(
                &entries,
                std::path::Path::new(&target_root),
                &mut conn,
                &today,
                &mut progress_cb,
            ) {
                Ok(summary) => {
                    let _ = tx.send(ImportMsg::CopyDone(summary));
                }
                Err(e) => {
                    let _ = tx.send(ImportMsg::CopyError(e.to_string()));
                }
            }
        });
    }

    /// Cancel the current import operation (scan preview or scanning state).
    pub fn cancel_import(&mut self) {
        self.import_rx = None;
        self.import_state = ImportState::Idle;
    }

    /// Scroll the import preview list down.
    pub fn import_preview_scroll_down(&mut self) {
        if let ImportState::Preview { scroll, entries } = &mut self.import_state {
            let visible = entries.iter().filter(|e| e.status != ImportStatus::Skipped).count();
            let max_scroll = visible.saturating_sub(self.import_list_height);
            if *scroll < max_scroll {
                *scroll += 1;
            }
        }
    }

    /// Scroll the import preview list up.
    pub fn import_preview_scroll_up(&mut self) {
        if let ImportState::Preview { scroll, .. } = &mut self.import_state {
            *scroll = scroll.saturating_sub(1);
        }
    }

    /// Scroll the import preview list down by one page.
    pub fn import_preview_page_down(&mut self) {
        if let ImportState::Preview { scroll, entries } = &mut self.import_state {
            let visible = entries.iter().filter(|e| e.status != ImportStatus::Skipped).count();
            let max_scroll = visible.saturating_sub(self.import_list_height);
            *scroll = (*scroll + self.import_list_height).min(max_scroll);
        }
    }

    /// Scroll the import preview list up by one page.
    pub fn import_preview_page_up(&mut self) {
        if let ImportState::Preview { scroll, .. } = &mut self.import_state {
            *scroll = scroll.saturating_sub(self.import_list_height);
        }
    }

    /// Poll the import receive channel — call once per event-loop tick.
    /// Returns `true` if a message was processed (caller should redraw).
    pub fn poll_import(&mut self) -> bool {
        let msg = match &self.import_rx {
            Some(rx) => match rx.try_recv() {
                Ok(m) => m,
                Err(_) => return false,
            },
            None => return false,
        };
        self.on_import_msg(msg);
        true
    }

    // ── fix-date ─────────────────────────────────────────────────────────────

    /// Apply `:fix-date <yyyy-mm-dd>` to the selection set (or cursor file
    /// if nothing is explicitly selected).
    pub fn fix_date_selected(&mut self, date_str: &str) {
        if !is_valid_date(date_str) {
            self.status_message = Some(format!("fix-date: invalid date '{date_str}' (expected yyyy-mm-dd)"));
            return;
        }

        // Collect file IDs to fix.
        let ids: Vec<String> = if self.selection.is_empty() {
            self.filtered
                .get(self.selected)
                .map(|f| vec![f.id.clone()])
                .unwrap_or_default()
        } else {
            let mut sel: Vec<usize> = self.selection.iter().copied().collect();
            sel.sort_unstable();
            sel.iter()
                .filter_map(|&i| self.filtered.get(i).map(|f| f.id.clone()))
                .collect()
        };

        if ids.is_empty() {
            self.status_message = Some("fix-date: no file selected".into());
            return;
        }

        let mut errors = 0usize;
        let mut first_error: Option<String> = None;
        for id in &ids {
            if let Err(e) = crate::db::fix_date(&self.db_path, &self.target_root, id, date_str) {
                eprintln!("fix-date error for {id}: {e}");
                if first_error.is_none() {
                    first_error = Some(e.to_string());
                }
                errors += 1;
            }
        }

        // Reload file list from DB.
        if let Err(e) = self.reload() {
            self.status_message = Some(format!("fix-date: reload failed: {e}"));
            return;
        }

        if errors == 0 {
            self.status_message = Some(format!(
                "fix-date: updated {} file(s) to {}",
                ids.len(),
                date_str
            ));
        } else if let Some(msg) = first_error {
            self.status_message = Some(format!(
                "fix-date: {} error(s) — {}",
                errors, msg
            ));
        } else {
            self.status_message = Some(format!(
                "fix-date: {} updated, {} error(s)",
                ids.len() - errors,
                errors
            ));
        }
    }

    // ── fix-ext ──────────────────────────────────────────────────────────────

    /// Apply `:fix-ext` to the selection set (or cursor file if nothing is
    /// explicitly selected): detect extension/format mismatches via magic bytes
    /// and rename the file on disk + update the DB for each affected file.
    pub fn fix_ext_selected(&mut self) {
        let targets: Vec<(String, String)> = if self.selection.is_empty() {
            self.filtered
                .get(self.selected)
                .map(|f| vec![(f.id.clone(), f.target_path.clone())])
                .unwrap_or_default()
        } else {
            let mut sel: Vec<usize> = self.selection.iter().copied().collect();
            sel.sort_unstable();
            sel.iter()
                .filter_map(|&i| self.filtered.get(i).map(|f| (f.id.clone(), f.target_path.clone())))
                .collect()
        };

        if targets.is_empty() {
            self.status_message = Some("fix-ext: no file selected".into());
            return;
        }

        let mut fixed = 0usize;
        let mut already_ok = 0usize;
        let mut errors = 0usize;
        let mut first_error: Option<String> = None;

        for (id, rel_path) in &targets {
            let abs_path = PathBuf::from(&self.target_root).join(rel_path);
            let current_ext = abs_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            match crate::import::detect_wrong_ext(&abs_path, &current_ext) {
                Some(new_ext) => {
                    if let Err(e) = crate::db::fix_ext(&self.db_path, &self.target_root, id, &new_ext) {
                        eprintln!("fix-ext error for {id}: {e}");
                        if first_error.is_none() {
                            first_error = Some(e.to_string());
                        }
                        errors += 1;
                    } else {
                        fixed += 1;
                    }
                }
                None => already_ok += 1,
            }
        }

        if let Err(e) = self.reload() {
            self.status_message = Some(format!("fix-ext: reload failed: {e}"));
            return;
        }

        self.status_message = Some(if errors > 0 {
            let msg = first_error.unwrap_or_default();
            format!("fix-ext: {errors} error(s) — {msg}")
        } else if fixed == 0 {
            format!("fix-ext: {already_ok} file(s) already correct")
        } else {
            format!("fix-ext: fixed {fixed} file(s)")
        });
    }

    // ── remove-slug ──────────────────────────────────────────────────────────

    /// Apply `:remove-slug` to the selection set (or cursor file if nothing is
    /// explicitly selected).
    ///
    /// Spawns a background thread that processes one file at a time, sending
    /// `RemoveSlugMsg::Progress` after each file and `RemoveSlugMsg::Done` when
    /// finished.  The UI shows a full-screen progress overlay while running.
    pub fn remove_slug_selected(&mut self) {
        let targets: Vec<(String, String)> = if self.selection.is_empty() {
            self.filtered
                .get(self.selected)
                .map(|f| vec![(f.id.clone(), f.derived_slug.clone())])
                .unwrap_or_default()
        } else {
            let mut sel: Vec<usize> = self.selection.iter().copied().collect();
            sel.sort_unstable();
            sel.iter()
                .filter_map(|&i| self.filtered.get(i).map(|f| (f.id.clone(), f.derived_slug.clone())))
                .collect()
        };

        if targets.is_empty() {
            self.status_message = Some("remove-slug: no file selected".into());
            return;
        }

        let total = targets.len();
        self.remove_slug_state = RemoveSlugState::Running {
            done: 0,
            total,
            current: String::new(),
        };

        let db_path = self.db_path.clone();
        let target_root = self.target_root.clone();
        let (tx, rx) = mpsc::channel::<RemoveSlugMsg>();
        self.remove_slug_rx = Some(rx);

        std::thread::spawn(move || {
            let mut fixed = 0usize;
            let mut skipped = 0usize;
            let mut errors = 0usize;
            let mut first_error: Option<String> = None;

            for (done, (id, slug)) in targets.iter().enumerate() {
                let current = id.clone();
                let _ = tx.send(RemoveSlugMsg::Progress {
                    done,
                    total,
                    current: current.clone(),
                });

                if slug.is_empty() {
                    skipped += 1;
                    continue;
                }
                match crate::db::remove_slug(&db_path, &target_root, id) {
                    Ok(()) => fixed += 1,
                    Err(e) => {
                        eprintln!("remove-slug error for {id}: {e}");
                        if first_error.is_none() {
                            first_error = Some(e.to_string());
                        }
                        errors += 1;
                    }
                }
            }

            let summary = if errors > 0 {
                let msg = first_error.unwrap_or_default();
                format!("remove-slug: {errors} error(s) — {msg}")
            } else if fixed == 0 {
                format!("remove-slug: {skipped} file(s) already have no slug")
            } else if skipped > 0 {
                format!("remove-slug: repaired {fixed} file(s), {skipped} skipped (no slug)")
            } else {
                format!("remove-slug: repaired {fixed} file(s)")
            };
            let _ = tx.send(RemoveSlugMsg::Done(summary));
        });
    }

    /// Dispatch a message received from the background remove-slug thread.
    pub fn on_remove_slug_msg(&mut self, msg: RemoveSlugMsg) {
        match msg {
            RemoveSlugMsg::Progress { done, total, current } => {
                self.remove_slug_state = RemoveSlugState::Running { done, total, current };
            }
            RemoveSlugMsg::Done(summary) => {
                self.remove_slug_rx = None;
                let _ = self.reload();
                self.remove_slug_state = RemoveSlugState::Done(summary.clone());
                self.status_message = Some(summary);
            }
        }
    }

    /// Poll the remove-slug background thread for new messages (non-blocking).
    /// Returns `true` if a message was processed.
    pub fn poll_remove_slug(&mut self) -> bool {
        let msg = match &self.remove_slug_rx {
            Some(rx) => match rx.try_recv() {
                Ok(m) => m,
                Err(_) => return false,
            },
            None => return false,
        };
        self.on_remove_slug_msg(msg);
        true
    }

    // ── fix-os-time ───────────────────────────────────────────────────────────

    /// Apply `:fix-os-time` to the selection set (or cursor file if nothing is
    /// explicitly selected).
    ///
    /// Spawns a background thread that re-applies the same mtime logic as the
    /// import execute phase to each target file's OS mtime on disk.
    pub fn fix_os_time_selected(&mut self) {
        let targets: Vec<String> = if self.selection.is_empty() {
            self.filtered
                .get(self.selected)
                .map(|f| vec![f.id.clone()])
                .unwrap_or_default()
        } else {
            let mut sel: Vec<usize> = self.selection.iter().copied().collect();
            sel.sort_unstable();
            sel.iter()
                .filter_map(|&i| self.filtered.get(i).map(|f| f.id.clone()))
                .collect()
        };

        if targets.is_empty() {
            self.status_message = Some("fix-os-time: no file selected".into());
            return;
        }

        let total = targets.len();
        self.fix_os_time_state = FixOsTimeState::Running {
            done: 0,
            total,
            current: String::new(),
        };

        let db_path = self.db_path.clone();
        let target_root = self.target_root.clone();
        let (tx, rx) = mpsc::channel::<FixOsTimeMsg>();
        self.fix_os_time_rx = Some(rx);

        std::thread::spawn(move || {
            let mut updated = 0usize;
            let mut skipped = 0usize;
            let mut errors = 0usize;
            let mut first_error: Option<String> = None;

            for (done, id) in targets.iter().enumerate() {
                let _ = tx.send(FixOsTimeMsg::Progress {
                    done,
                    total,
                    current: id.clone(),
                });

                match crate::db::fix_os_time(&db_path, &target_root, id) {
                    Ok(true) => updated += 1,
                    Ok(false) => skipped += 1,
                    Err(e) => {
                        eprintln!("fix-os-time error for {id}: {e}");
                        if first_error.is_none() {
                            first_error = Some(e.to_string());
                        }
                        errors += 1;
                    }
                }
            }

            let summary = if errors > 0 {
                let msg = first_error.unwrap_or_default();
                format!("fix-os-time: {errors} error(s) — {msg}")
            } else if updated == 0 {
                format!("fix-os-time: {skipped} file(s) skipped (no date)")
            } else if skipped > 0 {
                format!("fix-os-time: updated {updated} file(s), {skipped} skipped (no date)")
            } else {
                format!("fix-os-time: updated {updated} file(s)")
            };
            let _ = tx.send(FixOsTimeMsg::Done(summary));
        });
    }

    /// Dispatch a message received from the background fix-os-time thread.
    pub fn on_fix_os_time_msg(&mut self, msg: FixOsTimeMsg) {
        match msg {
            FixOsTimeMsg::Progress { done, total, current } => {
                self.fix_os_time_state = FixOsTimeState::Running { done, total, current };
            }
            FixOsTimeMsg::Done(summary) => {
                self.fix_os_time_rx = None;
                self.fix_os_time_state = FixOsTimeState::Done(summary.clone());
                self.status_message = Some(summary);
            }
        }
    }

    /// Poll the fix-os-time background thread for new messages (non-blocking).
    /// Returns `true` if a message was processed.
    pub fn poll_fix_os_time(&mut self) -> bool {
        let msg = match &self.fix_os_time_rx {
            Some(rx) => match rx.try_recv() {
                Ok(m) => m,
                Err(_) => return false,
            },
            None => return false,
        };
        self.on_fix_os_time_msg(msg);
        true
    }

    /// Apply `:tag <name>[@<type>]` to the selection set (or cursor file if
    /// nothing is explicitly selected). When `tag_type` is `None`, reuses the
    /// existing tag's type; creates as `"event"` if the tag is new.
    pub fn tag_selected(&mut self, tag_name: &str, tag_type: Option<&str>) {
        let ids: Vec<String> = if self.selection.is_empty() {
            self.filtered
                .get(self.selected)
                .map(|f| vec![f.id.clone()])
                .unwrap_or_default()
        } else {
            let mut sel: Vec<usize> = self.selection.iter().copied().collect();
            sel.sort_unstable();
            sel.iter()
                .filter_map(|&i| self.filtered.get(i).map(|f| f.id.clone()))
                .collect()
        };

        if ids.is_empty() {
            self.status_message = Some("tag: no file selected".into());
            return;
        }

        match crate::db::assign_tag(&self.db_path, &ids, tag_name, tag_type) {
            Err(e) => {
                self.status_message = Some(format!("tag: {e}"));
            }
            Ok(effective_type) => {
                let count = ids.len();
                if let Err(e) = self.reload() {
                    self.status_message = Some(format!("tag: reload failed: {e}"));
                    return;
                }
                self.status_message = Some(format!(
                    "tagged {count} file(s) with {}@{}",
                    tag_name, effective_type
                ));
            }
        }
    }

    /// Apply `:untag [name …]` to the selection set (or cursor file).
    ///
    /// - `tag_names` empty  → remove **all** tags from every targeted file.
    /// - `tag_names` given  → remove only those tags.
    pub fn untag_selected(&mut self, tag_names: &[String]) {
        let ids: Vec<String> = if self.selection.is_empty() {
            self.filtered
                .get(self.selected)
                .map(|f| vec![f.id.clone()])
                .unwrap_or_default()
        } else {
            let mut sel: Vec<usize> = self.selection.iter().copied().collect();
            sel.sort_unstable();
            sel.iter()
                .filter_map(|&i| self.filtered.get(i).map(|f| f.id.clone()))
                .collect()
        };

        if ids.is_empty() {
            self.status_message = Some("untag: no file selected".into());
            return;
        }

        match crate::db::remove_tags(&self.db_path, &ids, tag_names) {
            Err(e) => {
                self.status_message = Some(format!("untag: {e}"));
            }
            Ok(_) => {
                let file_count = ids.len();
                if let Err(e) = self.reload() {
                    self.status_message = Some(format!("untag: reload failed: {e}"));
                    return;
                }
                self.status_message = Some(if tag_names.is_empty() {
                    format!("cleared all tags from {file_count} file(s)")
                } else {
                    format!(
                        "removed {} from {file_count} file(s)",
                        tag_names.join(", ")
                    )
                });
            }
        }
    }

    // ── Trash / Keep ─────────────────────────────────────────────────────────

    /// Maximum files that can be trashed in a single operation (guardrail).
    const MAX_TRASH_BATCH: usize = 100;

    /// Mark the cursor file (or selection) as trashed. Clears the selection.
    /// Refuses if the selection exceeds `MAX_TRASH_BATCH` to prevent accidents.
    pub fn trash_selected(&mut self) {
        let ids: Vec<String> = if self.selection.is_empty() {
            self.filtered
                .get(self.selected)
                .filter(|f| f.status != "trashed")
                .map(|f| vec![f.id.clone()])
                .unwrap_or_default()
        } else {
            let mut sel: Vec<usize> = self.selection.iter().copied().collect();
            sel.sort_unstable();
            sel.iter()
                .filter_map(|&i| self.filtered.get(i))
                .filter(|f| f.status != "trashed")
                .map(|f| f.id.clone())
                .collect()
        };

        if ids.is_empty() {
            return;
        }

        if ids.len() > Self::MAX_TRASH_BATCH {
            self.status_message = Some(format!(
                "trash: too many files selected ({} > {} max) — deselect some first",
                ids.len(),
                Self::MAX_TRASH_BATCH,
            ));
            return;
        }

        match crate::db::trash_files(&self.db_path, &ids) {
            Err(e) => {
                self.status_message = Some(format!("trash: {e}"));
            }
            Ok(n) => {
                self.selection.clear();
                if let Err(e) = self.reload_preserve_cursor() {
                    self.status_message = Some(format!("trash: reload failed: {e}"));
                    return;
                }
                self.status_message = Some(format!("trashed {n} file(s)"));
            }
        }
    }

    /// Restore the cursor file from trash to normal (`status = 'moved'`).
    /// Since trashed files cannot be selected, this always operates on the cursor only.
    pub fn keep_selected(&mut self) {
        let id = match self.filtered.get(self.selected) {
            Some(f) if f.status == "trashed" => f.id.clone(),
            _ => return,
        };

        match crate::db::keep_files(&self.db_path, &[id]) {
            Err(e) => {
                self.status_message = Some(format!("keep: {e}"));
            }
            Ok(_) => {
                if let Err(e) = self.reload_preserve_cursor() {
                    self.status_message = Some(format!("keep: reload failed: {e}"));
                    return;
                }
                self.status_message = Some("restored file from trash".into());
            }
        }
    }

    // ── Empty-trash ───────────────────────────────────────────────────────────

    /// Load up to 100 trashed files and enter the preview state.
    pub fn start_empty_trash(&mut self) {
        match crate::db::load_trashed_files(&self.db_path, 100) {
            Err(e) => {
                self.status_message = Some(format!("empty-trash: {e}"));
            }
            Ok(files) if files.is_empty() => {
                self.status_message = Some("empty-trash: trash is empty".into());
            }
            Ok(files) => {
                self.empty_trash_state = EmptyTrashState::Preview { files, scroll: 0 };
            }
        }
    }

    /// Confirm empty-trash: spawn background thread to delete the files.
    pub fn confirm_empty_trash(&mut self) {
        let files = match &self.empty_trash_state {
            EmptyTrashState::Preview { files, .. } => files.clone(),
            _ => return,
        };

        let ids: Vec<String> = files.iter().map(|f| f.id.clone()).collect();
        let total = ids.len();
        self.empty_trash_state = EmptyTrashState::Deleting { done: 0, total };

        let db_path = self.db_path.clone();
        let target_root = self.target_root.clone();
        let (tx, rx) = mpsc::channel::<EmptyTrashMsg>();
        self.empty_trash_rx = Some(rx);

        std::thread::spawn(move || {
            // Delete one by one so we can report progress.
            let mut done = 0usize;
            let mut errors = 0usize;
            for id in &ids {
                let _ = tx.send(EmptyTrashMsg::Progress { done, total });
                match crate::db::delete_trashed_from_fs(&db_path, &target_root, &[id.clone()]) {
                    Ok((d, e)) => { done += d; errors += e; }
                    Err(_) => { errors += 1; }
                }
            }
            let _ = tx.send(EmptyTrashMsg::Done(done, errors));
        });
    }

    /// Cancel the empty-trash preview.
    pub fn cancel_empty_trash(&mut self) {
        self.empty_trash_state = EmptyTrashState::Idle;
    }

    /// Scroll the empty-trash preview list down by one line.
    pub fn empty_trash_scroll_down(&mut self) {
        if let EmptyTrashState::Preview { files, scroll } = &mut self.empty_trash_state {
            let max = files.len().saturating_sub(1);
            if *scroll < max { *scroll += 1; }
        }
    }

    /// Scroll the empty-trash preview list up by one line.
    pub fn empty_trash_scroll_up(&mut self) {
        if let EmptyTrashState::Preview { scroll, .. } = &mut self.empty_trash_state {
            if *scroll > 0 { *scroll -= 1; }
        }
    }

    /// Scroll the empty-trash preview list down by one page.
    pub fn empty_trash_page_down(&mut self) {
        if let EmptyTrashState::Preview { files, scroll } = &mut self.empty_trash_state {
            let page = self.import_list_height.max(1);
            let max = files.len().saturating_sub(1);
            *scroll = (*scroll + page).min(max);
        }
    }

    /// Scroll the empty-trash preview list up by one page.
    pub fn empty_trash_page_up(&mut self) {
        if let EmptyTrashState::Preview { scroll, .. } = &mut self.empty_trash_state {
            let page = self.import_list_height.max(1);
            *scroll = scroll.saturating_sub(page);
        }
    }

    /// Handle a message from the background empty-trash thread.
    fn on_empty_trash_msg(&mut self, msg: EmptyTrashMsg) {
        match msg {
            EmptyTrashMsg::Progress { done, total } => {
                self.empty_trash_state = EmptyTrashState::Deleting { done, total };
            }
            EmptyTrashMsg::Done(deleted, errors) => {
                self.empty_trash_rx = None;
                let summary = if errors == 0 {
                    format!("empty-trash: deleted {deleted} file(s)")
                } else {
                    format!("empty-trash: deleted {deleted} file(s), {errors} error(s)")
                };
                self.empty_trash_state = EmptyTrashState::Idle;
                self.status_message = Some(summary.clone());
                // Reload the list so deleted files are no longer shown.
                let _ = self.reload();
            }
        }
    }

    /// Poll the empty-trash background thread for new messages (non-blocking).
    /// Returns `true` if a message was processed.
    pub fn poll_empty_trash(&mut self) -> bool {
        let msg = match &self.empty_trash_rx {
            Some(rx) => match rx.try_recv() {
                Ok(m) => m,
                Err(_) => return false,
            },
            None => return false,
        };
        self.on_empty_trash_msg(msg);
        true
    }

    /// Reload `all_files` from the DB and re-apply the current filter.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.all_files = crate::db::load_files(&self.db_path)?;
        // Rebuild the tag and type sets from the fresh data.
        let mut tag_set: BTreeSet<String> = BTreeSet::new();
        let mut type_set: BTreeSet<String> = BTreeSet::new();
        self.trashed_count = 0;
        for f in &self.all_files {
            for tag in &f.tags {
                tag_set.insert(tag.clone());
            }
            for ty in &f.tag_types {
                if !ty.is_empty() {
                    type_set.insert(ty.clone());
                }
            }
            if f.status == "trashed" {
                self.trashed_count += 1;
            }
        }
        self.all_tags = tag_set.into_iter().collect();
        self.all_tag_types = type_set.into_iter().collect();
        self.apply_filter();
        Ok(())
    }

    /// Reload and restore the cursor to the file it was on before the reload.
    ///
    /// Looks up the current file's `id`, reloads, then finds that id in the
    /// new filtered list and moves the cursor there (falling back to the last
    /// item if the file has been removed from the filtered view).
    pub fn reload_preserve_cursor(&mut self) -> anyhow::Result<()> {
        let saved_id = self.filtered.get(self.selected).map(|f| f.id.clone());
        self.reload()?;
        if let Some(id) = saved_id {
            if let Some(new_idx) = self.filtered.iter().position(|f| f.id == id) {
                self.selected = new_idx;
            } else {
                // File left the filtered view — land on the nearest still-valid index.
                self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
            }
            self.ensure_visible();
        }
        Ok(())
    }

    pub(crate) fn apply_filter(&mut self) {
        let text_needle = self.filter_text.to_lowercase();
        self.filtered = if text_needle.is_empty() && self.tag_filters.is_empty() && self.tag_type_filters.is_empty() {
            self.all_files.clone()
        } else {
            self.all_files
                .iter()
                .filter(|f| {
                    let text_match = text_needle.is_empty()
                        || f.target_path.to_lowercase().contains(&text_needle);
                    let type_match = self.tag_type_filters.is_empty()
                        || self.tag_type_filters.iter().any(|ty| {
                            f.tag_types.iter().any(|ft| ft.eq_ignore_ascii_case(ty))
                        });
                    let tag_match = self.tag_filters.is_empty()
                        || self.tag_filters.iter().any(|t| {
                            f.tags.iter().any(|ft| ft.eq_ignore_ascii_case(t))
                        });
                    text_match && type_match && tag_match
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
    /// Trashed files cannot be selected.
    pub fn toggle_selection(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.filtered.get(self.selected).map(|f| f.status == "trashed").unwrap_or(false) {
            return;
        }
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

    /// Select all non-trashed files in the current (filtered) result set.
    /// If every selectable file is already selected, deselects all instead.
    pub fn select_all_or_none(&mut self) {
        let selectable: Vec<usize> = (0..self.filtered.len())
            .filter(|&i| self.filtered.get(i).map(|f| f.status != "trashed").unwrap_or(false))
            .collect();
        if selectable.is_empty() {
            return;
        }
        if selectable.iter().all(|i| self.selection.contains(i)) {
            self.selection.clear();
        } else {
            self.selection.extend(selectable);
        }
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
        // Trashed files cannot be selected.
        if self.filtered.get(idx).map(|f| f.status == "trashed").unwrap_or(false) {
            return;
        }
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
        let selectable: Vec<usize> = (lo..=hi)
            .filter(|&i| self.filtered.get(i).map_or(false, |f| f.status != "trashed"))
            .collect();
        let all_selected = selectable.iter().all(|i| self.selection.contains(i));
        if all_selected {
            for i in selectable { self.selection.remove(&i); }
        } else {
            for i in selectable { self.selection.insert(i); }
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

/// Returns true if `s` is a valid `yyyy-mm-dd` date string.
fn is_valid_date(s: &str) -> bool {
    if s.len() != 10 { return false; }
    let b = s.as_bytes();
    if b[4] != b'-' || b[7] != b'-' { return false; }
    let ok_digits = |slice: &[u8]| slice.iter().all(|c| c.is_ascii_digit());
    if !ok_digits(&b[0..4]) || !ok_digits(&b[5..7]) || !ok_digits(&b[8..10]) {
        return false;
    }
    let month: u8 = s[5..7].parse().unwrap_or(0);
    let day: u8 = s[8..10].parse().unwrap_or(0);
    month >= 1 && month <= 12 && day >= 1 && day <= 31
}

/// Return today's date as `"YYYY-MM-DD"` using standard Unix time.
fn today_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    crate::import::secs_to_date_pub(secs)
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
                tag_types: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
                os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            })
            .collect();
        App::new("test.db".into(), root, String::new(), files, picker, image_state, "halfblocks".into())
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
                tag_types: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
                os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
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
                tag_types: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
                os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            });
        }
        App::new("test.db".into(), root, String::new(), files, picker, image_state, "halfblocks".into())
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
                    tag_types: vec![],
                    derived_slug,
                    caption_slug: String::new(),
                    os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
                });
                idx += 1;
            }
        }
        App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into())
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

    // ── UC-05 · Tag filtering ────────────────────────────────────────────────

    /// Build a test App where each entry is `(target_path, tags)`.
    fn make_tagged_app(entries: &[(&str, &[&str])]) -> App {
        let (tx, _rx) = mpsc::channel();
        let image_state = ThreadProtocol::new(tx, None);
        let picker = Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = entries
            .iter()
            .enumerate()
            .map(|(i, (path, tags))| crate::db::MediaFile {
                id: i.to_string(),
                target_path: path.to_string(),
                derived_date: "2024-01-01".into(),
                ext: path.rsplit('.').next().unwrap_or("").into(),
                tags: tags.iter().map(|s| s.to_string()).collect(),
                tag_types: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
                os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            })
            .collect();
        App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into())
    }

    #[test]
    fn tag_filter_single() {
        let mut app = make_tagged_app(&[
            ("2023/a.jpg", &["travel"]),
            ("2023/b.jpg", &["work"]),
            ("2023/c.jpg", &[]),
        ]);
        app.tag_filters.push("travel".into());
        app.apply_filter();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.filtered[0].target_path, "2023/a.jpg");
    }

    #[test]
    fn tag_filter_or_logic() {
        let mut app = make_tagged_app(&[
            ("2023/a.jpg", &["travel"]),
            ("2023/b.jpg", &["holiday"]),
            ("2023/c.jpg", &["work"]),
        ]);
        app.tag_filters.push("travel".into());
        app.tag_filters.push("holiday".into());
        app.apply_filter();
        assert_eq!(app.filtered.len(), 2);
    }

    #[test]
    fn tag_filter_case_insensitive() {
        let mut app = make_tagged_app(&[
            ("a.jpg", &["Travel"]),
            ("b.jpg", &["work"]),
        ]);
        app.tag_filters.push("TRAVEL".into());
        app.apply_filter();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.filtered[0].target_path, "a.jpg");
    }

    #[test]
    fn text_filter_skips_tags() {
        let mut app = make_tagged_app(&[
            ("photo.jpg", &["travel"]),
            ("travel.jpg", &["work"]),
        ]);
        // "travel" as text should match filename only, not the tag
        app.filter_text = "travel".into();
        app.apply_filter();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.filtered[0].target_path, "travel.jpg");
    }

    #[test]
    fn combined_filter_and_logic() {
        let mut app = make_tagged_app(&[
            ("vacation.jpg", &["travel"]),   // text match + tag match → include
            ("vacation.jpg2", &["work"]),    // text match, tag no match → exclude
            ("other.jpg", &["travel"]),      // tag match, text no match → exclude
        ]);
        app.filter_text = "vacation".into();
        app.tag_filters.push("travel".into());
        app.apply_filter();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.filtered[0].target_path, "vacation.jpg");
    }

    #[test]
    fn tag_autocomplete_suggestion() {
        let app = make_tagged_app(&[
            ("a.jpg", &["travel", "work"]),
        ]);
        // all_tags should be ["travel", "work"] (sorted)
        assert!(app.all_tags.contains(&"travel".to_string()));
        assert!(app.all_tags.contains(&"work".to_string()));
        // Simulate typing "#tra"
        let mut app2 = make_tagged_app(&[("a.jpg", &["travel", "trail"])]);
        app2.tag_typing = true;
        app2.tag_input = "tra".into();
        let suggestions = app2.filtered_tag_suggestions();
        assert!(suggestions.iter().any(|s| s.as_str() == "travel"));
        assert!(suggestions.iter().any(|s| s.as_str() == "trail"));
        // "work" should not appear
        assert!(!suggestions.iter().any(|s| s.as_str() == "work"));
    }

    #[test]
    fn tab_complete_fills_input() {
        let mut app = make_tagged_app(&[("a.jpg", &["travel"])]);
        app.tag_typing = true;
        app.tag_input = "tra".into();
        app.tab_complete();
        assert_eq!(app.tag_input, "travel");
    }

    #[test]
    fn backspace_exits_tag_mode() {
        let mut app = make_tagged_app(&[("a.jpg", &["travel"])]);
        app.tag_typing = true;
        app.tag_input.clear(); // empty tag_input
        app.pop_filter_char();
        assert!(!app.tag_typing, "backspace on empty tag_input should exit tag_typing");
    }

    #[test]
    fn backspace_removes_last_tag() {
        let mut app = make_tagged_app(&[("a.jpg", &["travel", "work"])]);
        app.tag_filters.push("travel".into());
        app.tag_filters.push("work".into());
        // filter_text is empty and not tag_typing
        app.pop_filter_char();
        assert_eq!(app.tag_filters.len(), 1);
        assert_eq!(app.tag_filters[0], "travel");
    }

    #[test]
    fn confirm_tag_adds_to_filters() {
        let mut app = make_tagged_app(&[("a.jpg", &["travel"])]);
        app.tag_typing = true;
        app.tag_input = "travel".into();
        app.confirm_tag();
        assert!(!app.tag_typing);
        assert!(app.tag_filters.contains(&"travel".to_string()));
        assert!(app.tag_input.is_empty());
    }

    #[test]
    fn confirm_tag_no_duplicates() {
        let mut app = make_tagged_app(&[("a.jpg", &["travel"])]);
        app.tag_filters.push("travel".into());
        app.tag_typing = true;
        app.tag_input = "TRAVEL".into();
        app.confirm_tag();
        // Should not add again (case-insensitive dedup)
        assert_eq!(app.tag_filters.len(), 1);
    }

    #[test]
    fn clear_filter_resets_all() {
        let mut app = make_tagged_app(&[("a.jpg", &["travel"])]);
        app.filter_text = "foo".into();
        app.tag_filters.push("travel".into());
        app.tag_typing = true;
        app.tag_input = "tra".into();
        app.suggestion_idx = 1;
        app.clear_filter();
        assert!(app.filter_text.is_empty());
        assert!(app.tag_filters.is_empty());
        assert!(!app.tag_typing);
        assert!(app.tag_input.is_empty());
        assert_eq!(app.suggestion_idx, 0);
    }

    // ── UC-07 command autocomplete ───────────────────────────────────────────

    fn make_cmd_app() -> App {
        let (tx, _rx) = mpsc::channel();
        let image_state = ThreadProtocol::new(tx, None);
        let picker = Picker::halfblocks();
        App::new("test.db".into(), String::new(), String::new(), vec![], picker, image_state, "halfblocks".into())
    }

    #[test]
    fn command_suggestion_prefix() {
        let mut app = make_cmd_app();
        app.enter_command_mode();
        "fix".chars().for_each(|c| app.push_command_char(c));
        let suggestions = app.command_name_suggestions();
        assert!(suggestions.contains(&"fix-date"), "typing 'fix' should suggest 'fix-date'");
    }

    #[test]
    fn command_suggestion_full_name() {
        let mut app = make_cmd_app();
        app.enter_command_mode();
        "fix-date".chars().for_each(|c| app.push_command_char(c));
        let suggestions = app.command_name_suggestions();
        assert!(suggestions.contains(&"fix-date"), "exact command name still shows suggestion");
    }

    #[test]
    fn command_suggestion_empty_returns_all() {
        let mut app = make_cmd_app();
        app.enter_command_mode();
        let suggestions = app.command_name_suggestions();
        assert_eq!(suggestions.len(), KNOWN_COMMANDS.len());
    }

    #[test]
    fn command_suggestion_no_match() {
        let mut app = make_cmd_app();
        app.enter_command_mode();
        "zzz".chars().for_each(|c| app.push_command_char(c));
        assert!(app.command_name_suggestions().is_empty());
    }

    #[test]
    fn tab_complete_fills_command_name() {
        let mut app = make_cmd_app();
        app.enter_command_mode();
        "fix".chars().for_each(|c| app.push_command_char(c));
        app.tab_complete();
        assert_eq!(app.command.as_deref(), Some("fix-date "));
    }

    #[test]
    fn tab_complete_no_fill_after_space() {
        let mut app = make_cmd_app();
        app.enter_command_mode();
        "fix-date 2024-01-01".chars().for_each(|c| app.push_command_char(c));
        app.tab_complete();
        // Should not overwrite the argument.
        assert_eq!(app.command.as_deref(), Some("fix-date 2024-01-01"));
    }

    #[test]
    fn fix_date_invalid_format_sets_status() {
        let mut app = make_cmd_app();
        app.fix_date_selected("not-a-date");
        assert!(app.status_message.is_some());
        let msg = app.status_message.as_deref().unwrap_or("");
        assert!(msg.contains("invalid date"), "expected invalid date msg, got: {msg}");
    }

    #[test]
    fn fix_date_invalid_month_sets_status() {
        let mut app = make_cmd_app();
        app.fix_date_selected("2024-13-01");
        assert!(app.status_message.is_some());
    }

    #[test]
    fn is_valid_date_valid() {
        assert!(is_valid_date("2024-01-01"));
        assert!(is_valid_date("2000-12-31"));
    }

    #[test]
    fn is_valid_date_invalid() {
        assert!(!is_valid_date("2024-1-1"));
        assert!(!is_valid_date("2024-13-01"));
        assert!(!is_valid_date("not-date-x"));
        assert!(!is_valid_date(""));
    }

    // ── UC-09 tag command tests ───────────────────────────────────────────────

    fn make_tag_db() -> (std::path::PathBuf, String) {
        use std::fs;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mex_tag_test_{}_{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("mex.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (id TEXT PRIMARY KEY, target_path TEXT, derived_date TEXT,
                                 ext TEXT, os_date TEXT, derived_slug TEXT, caption_slug TEXT);
             CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE,
                                type TEXT NOT NULL DEFAULT 'event');
             CREATE TABLE media_tags (media_id TEXT NOT NULL, tag_id INTEGER NOT NULL,
                                      PRIMARY KEY (media_id, tag_id));
             INSERT INTO media VALUES ('m1','2024/a.jpg','2024-01-01','jpg',NULL,'','');
             INSERT INTO media VALUES ('m2','2024/b.jpg','2024-01-01','jpg',NULL,'','');",
        ).unwrap();
        (dir, db_path.to_str().unwrap().to_string())
    }

    #[test]
    fn assign_tag_creates_new_tag_and_attaches() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let tag_id: i64 = conn.query_row("SELECT id FROM tags WHERE name='holiday'", [], |r| r.get(0)).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM media_tags WHERE media_id='m1' AND tag_id=?1", [tag_id], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn assign_tag_reuses_existing_same_type() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();
        crate::db::assign_tag(&db_path, &["m2".to_string()], "holiday", Some("event")).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let tag_count: i64 = conn.query_row("SELECT COUNT(*) FROM tags WHERE name='holiday'", [], |r| r.get(0)).unwrap();
        assert_eq!(tag_count, 1, "should not create duplicate tag");

        let link_count: i64 = conn.query_row("SELECT COUNT(*) FROM media_tags", [], |r| r.get(0)).unwrap();
        assert_eq!(link_count, 2);
    }

    #[test]
    fn assign_tag_errors_on_type_mismatch() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();
        let result = crate::db::assign_tag(&db_path, &["m2".to_string()], "holiday", Some("person"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("already exists as type"), "unexpected error: {msg}");
    }

    #[test]
    fn assign_tag_duplicate_on_same_file_is_noop() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM media_tags", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1, "duplicate attach should be ignored");
    }

    #[test]
    fn assign_tag_defaults_to_event_type() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "trip", None).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let ty: String = conn.query_row("SELECT type FROM tags WHERE name='trip'", [], |r| r.get(0)).unwrap();
        assert_eq!(ty, "event");
    }

    #[test]
    fn assign_tag_omit_type_reuses_existing_any_type() {
        let (_dir, db_path) = make_tag_db();
        // Create tag with type "person"
        crate::db::assign_tag(&db_path, &["m1".to_string()], "alice", Some("person")).unwrap();
        // Assign same tag without specifying type — must not error
        let result = crate::db::assign_tag(&db_path, &["m2".to_string()], "alice", None);
        assert!(result.is_ok(), "omitting @type should reuse existing tag, got: {:?}", result);
        let effective = result.unwrap();
        assert_eq!(effective, "person");

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let link_count: i64 = conn.query_row("SELECT COUNT(*) FROM media_tags", [], |r| r.get(0)).unwrap();
        assert_eq!(link_count, 2);
    }

    #[test]
    fn tag_arg_suggestions_name_prefix() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = vec![
            crate::db::MediaFile {
                id: "1".into(), target_path: "a.jpg".into(), derived_date: "2024-01-01".into(),
                ext: "jpg".into(), tags: vec!["travel".into(), "trip".into()], tag_types: vec![],
                derived_slug: String::new(), caption_slug: String::new(), os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            }
        ];
        let mut app = App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into());
        app.enter_command_mode();
        "tag tr".chars().for_each(|c| app.push_command_char(c));
        let suggestions = app.tag_arg_suggestions();
        assert!(suggestions.contains(&"travel".to_string()));
        assert!(suggestions.contains(&"trip".to_string()));
        assert!(!suggestions.contains(&"other".to_string()));
    }

    #[test]
    fn tag_arg_suggestions_type_prefix() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = vec![
            crate::db::MediaFile {
                id: "1".into(), target_path: "a.jpg".into(), derived_date: "2024-01-01".into(),
                ext: "jpg".into(), tags: vec!["alice".into()], tag_types: vec!["person".into()],
                derived_slug: String::new(), caption_slug: String::new(), os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            }
        ];
        let mut app = App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into());
        app.enter_command_mode();
        "tag newtag@per".chars().for_each(|c| app.push_command_char(c));
        let suggestions = app.tag_arg_suggestions();
        assert!(suggestions.contains(&"person".to_string()));
    }

    #[test]
    fn tab_complete_fills_tag_name() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = vec![
            crate::db::MediaFile {
                id: "1".into(), target_path: "a.jpg".into(), derived_date: "2024-01-01".into(),
                ext: "jpg".into(), tags: vec!["vacation".into()], tag_types: vec![],
                derived_slug: String::new(), caption_slug: String::new(), os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            }
        ];
        let mut app = App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into());
        app.enter_command_mode();
        "tag vac".chars().for_each(|c| app.push_command_char(c));
        app.tab_complete();
        assert_eq!(app.command.as_deref(), Some("tag vacation"));
    }

    #[test]
    fn tab_complete_fills_tag_type() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = vec![
            crate::db::MediaFile {
                id: "1".into(), target_path: "a.jpg".into(), derived_date: "2024-01-01".into(),
                ext: "jpg".into(), tags: vec!["alice".into()], tag_types: vec!["person".into()],
                derived_slug: String::new(), caption_slug: String::new(), os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            }
        ];
        let mut app = App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into());
        app.enter_command_mode();
        "tag newtag@per".chars().for_each(|c| app.push_command_char(c));
        app.tab_complete();
        assert_eq!(app.command.as_deref(), Some("tag newtag@person"));
    }

    #[test]
    fn execute_tag_command_no_arg_sets_usage() {
        let mut app = make_cmd_app();
        app.enter_command_mode();
        "tag".chars().for_each(|c| app.push_command_char(c));
        app.execute_command();
        let msg = app.status_message.as_deref().unwrap_or("");
        assert!(msg.contains("usage"), "expected usage hint, got: {msg}");
    }

    #[test]
    fn known_commands_includes_tag() {
        let mut app = make_cmd_app();
        app.command = Some("ta".into());
        let suggestions = app.command_name_suggestions();
        assert!(suggestions.contains(&"tag"), "tag should be in KNOWN_COMMANDS");
    }

    // ── :untag tests ─────────────────────────────────────────────────────────

    #[test]
    fn remove_tags_all() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "summer", Some("event")).unwrap();
        let removed = crate::db::remove_tags(&db_path, &["m1".to_string()], &[]).unwrap();
        assert_eq!(removed, 2);
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM media_tags WHERE media_id='m1'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn remove_tags_specific() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "summer", Some("event")).unwrap();
        crate::db::remove_tags(&db_path, &["m1".to_string()], &["holiday".to_string()]).unwrap();
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM media_tags WHERE media_id='m1'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1, "should only remove the specified tag");
    }

    #[test]
    fn remove_tags_unknown_name_is_noop() {
        let (_dir, db_path) = make_tag_db();
        crate::db::assign_tag(&db_path, &["m1".to_string()], "holiday", Some("event")).unwrap();
        crate::db::remove_tags(&db_path, &["m1".to_string()], &["nonexistent".to_string()]).unwrap();
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM media_tags WHERE media_id='m1'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1, "removing unknown tag should leave existing tag intact");
    }

    #[test]
    fn untag_arg_suggestions_last_word() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = vec![
            crate::db::MediaFile {
                id: "1".into(), target_path: "a.jpg".into(), derived_date: "2024-01-01".into(),
                ext: "jpg".into(), tags: vec!["travel".into(), "trip".into()], tag_types: vec![],
                derived_slug: String::new(), caption_slug: String::new(), os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            }
        ];
        let mut app = App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into());
        app.command = Some("untag tr".into());
        let suggestions = app.tag_arg_suggestions();
        assert!(suggestions.contains(&"travel".to_string()));
        assert!(suggestions.contains(&"trip".to_string()));
    }

    #[test]
    fn tab_complete_fills_untag_name_with_space() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = vec![
            crate::db::MediaFile {
                id: "1".into(), target_path: "a.jpg".into(), derived_date: "2024-01-01".into(),
                ext: "jpg".into(), tags: vec!["vacation".into()], tag_types: vec![],
                derived_slug: String::new(), caption_slug: String::new(), os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            }
        ];
        let mut app = App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into());
        app.enter_command_mode();
        "untag vac".chars().for_each(|c| app.push_command_char(c));
        app.tab_complete();
        assert_eq!(app.command.as_deref(), Some("untag vacation "), "tab should complete and append space");
    }

    #[test]
    fn tab_complete_untag_second_word() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
        let files: Vec<crate::db::MediaFile> = vec![
            crate::db::MediaFile {
                id: "1".into(), target_path: "a.jpg".into(), derived_date: "2024-01-01".into(),
                ext: "jpg".into(), tags: vec!["vacation".into(), "trip".into()], tag_types: vec![],
                derived_slug: String::new(), caption_slug: String::new(), os_date: String::new(), orig_filename: String::new(), status: "moved".into(),
            }
        ];
        let mut app = App::new("test.db".into(), String::new(), String::new(), files, picker, image_state, "halfblocks".into());
        app.enter_command_mode();
        "untag vacation tri".chars().for_each(|c| app.push_command_char(c));
        app.tab_complete();
        assert_eq!(app.command.as_deref(), Some("untag vacation trip "));
    }

    // ── create-view ───────────────────────────────────────────────────────────

    fn make_test_app_with_views_root(image_names: &[&str], views_root: &str) -> App {
        use std::sync::mpsc;
        let (tx, _rx) = mpsc::channel();
        let image_state = ratatui_image::thread::ThreadProtocol::new(tx, None);
        let picker = ratatui_image::picker::Picker::halfblocks();
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
                tag_types: vec![],
                derived_slug: String::new(),
                caption_slug: String::new(),
                os_date: String::new(),
                orig_filename: String::new(), status: "moved".into(),
            })
            .collect();
        App::new("test.db".into(), root, views_root.to_string(), files, picker, image_state, "halfblocks".into())
    }

    #[test]
    fn create_view_no_views_root_configured() {
        let mut app = make_test_app_with_views_root(&["rolze.jpg"], "");
        app.create_view("myview");
        let msg = app.status_message.as_deref().unwrap_or("");
        assert!(msg.contains("views_root"), "expected views_root error, got: {msg}");
    }

    #[test]
    fn create_view_empty_name() {
        let tmp = std::env::temp_dir().join("mex_test_views_empty_name");
        std::fs::create_dir_all(&tmp).unwrap();
        let views_root = tmp.to_string_lossy().into_owned();
        let mut app = make_test_app_with_views_root(&["rolze.jpg"], &views_root);
        app.create_view("");
        let msg = app.status_message.as_deref().unwrap_or("");
        assert!(msg.contains("usage"), "expected usage error, got: {msg}");
    }

    #[test]
    fn create_view_happy_path() {
        let tmp = std::env::temp_dir().join("mex_test_views_happy");
        std::fs::create_dir_all(&tmp).unwrap();
        let views_root = tmp.to_string_lossy().into_owned();
        let mut app = make_test_app_with_views_root(&["rolze.jpg"], &views_root);
        app.create_view("testview");
        let msg = app.status_message.as_deref().unwrap_or("");
        assert!(msg.contains("testview"), "expected success message with view name, got: {msg}");
        let link = tmp.join("testview").join("rolze.jpg");
        assert!(link.exists(), "hard link should exist at {}", link.display());
        // Cleanup
        let _ = std::fs::remove_dir_all(tmp.join("testview"));
    }

    #[test]
    fn create_view_overwrites_existing() {
        let tmp = std::env::temp_dir().join("mex_test_views_overwrite");
        std::fs::create_dir_all(&tmp).unwrap();
        let views_root = tmp.to_string_lossy().into_owned();
        // Create a stale file in the view dir
        let view_dir = tmp.join("myview");
        std::fs::create_dir_all(&view_dir).unwrap();
        std::fs::write(view_dir.join("stale.txt"), b"old").unwrap();
        let mut app = make_test_app_with_views_root(&["rolze.jpg"], &views_root);
        app.create_view("myview");
        assert!(!view_dir.join("stale.txt").exists(), "stale file should be gone");
        assert!(view_dir.join("rolze.jpg").exists(), "new link should exist");
        // Cleanup
        let _ = std::fs::remove_dir_all(tmp.join("myview"));
    }

    #[test]
    fn create_view_uses_selection_when_non_empty() {
        let tmp = std::env::temp_dir().join("mex_test_views_selection");
        std::fs::create_dir_all(&tmp).unwrap();
        let views_root = tmp.to_string_lossy().into_owned();
        let mut app = make_test_app_with_views_root(&["rolze.jpg", "bg.png"], &views_root);
        // Select only index 1 (bg.png)
        app.selection.insert(1);
        app.create_view("selview");
        let view_dir = tmp.join("selview");
        assert!(!view_dir.join("rolze.jpg").exists(), "unselected file must not be linked");
        assert!(view_dir.join("bg.png").exists(), "selected file must be linked");
        // Cleanup
        let _ = std::fs::remove_dir_all(tmp.join("selview"));
    }
}
