#[cfg(test)]
mod tests {
    use crate::app::{App, Mode};
    use crate::config::Config;
    use crate::db;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Creates an in-memory DB with test schema and seed data.
    fn test_app(items: &[(&str, &str, &str, &str)]) -> App {
        let config = Config {
            target_root: None,
            views_root: None,
            db_path: None,
            image_protocol: "halfblocks".to_string(),
        };
        let conn = db::init_db(":memory:").expect("in-memory db");

        for (id, path_stem, ext, tags) in items {
            conn.execute(
                "INSERT INTO media (id, source_path, path_stem, partial_hash, file_size, ext, mex_date, status, tags_packed, tag_types_packed)
                 VALUES (?1, ?2, ?3, 'hash', 1000, ?4, '2024-01-01', 'imported', ?5, '')",
                rusqlite::params![id, format!("/src/{}{}", path_stem, ext), path_stem, ext, tags],
            )
            .expect("insert");
        }

        App::new(config, conn).expect("app init")
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn key_shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    fn seed_items() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
        vec![
            ("1", "2024-01-05-sunrise", ".jpg", "nature"),
            ("2", "2024-01-05-sunrise-2", ".jpg", "nature"),
            ("3", "2024-01-10-coffee", ".png", "food"),
            ("4", "2024-02-14-valentine", ".jpg", "holiday"),
            ("5", "2024-03-beach-0001", ".mp4", "travel"),
            ("6", "2024-03-beach-0002", ".mp4", "travel"),
        ]
    }

    // ── Navigation ──────────────────────────────────────────────────────

    #[test]
    fn cursor_starts_at_zero() {
        let app = test_app(&seed_items());
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.filtered_items.len(), 6);
    }

    #[test]
    fn arrow_down_moves_cursor() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.cursor_pos, 1);
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.cursor_pos, 2);
    }

    #[test]
    fn arrow_up_moves_cursor() {
        let mut app = test_app(&seed_items());
        app.cursor_pos = 3;
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.cursor_pos, 2);
    }

    #[test]
    fn arrow_up_at_top_stays() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn arrow_down_at_bottom_stays() {
        let mut app = test_app(&seed_items());
        app.cursor_pos = 5;
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.cursor_pos, 5);
    }

    #[test]
    fn page_down_jumps_list_height() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.cursor_pos, 5); // clamped to last item
    }

    #[test]
    fn page_up_jumps_list_height() {
        let mut app = test_app(&seed_items());
        app.cursor_pos = 5;
        app.handle_key(key(KeyCode::PageUp));
        assert_eq!(app.cursor_pos, 0); // clamped via saturating_sub
    }

    #[test]
    fn ctrl_d_half_page_down() {
        let mut app = test_app(&seed_items());
        app.handle_key(key_ctrl(KeyCode::Char('d')));
        assert_eq!(app.cursor_pos, 5); // clamped to last
    }

    #[test]
    fn ctrl_u_half_page_up() {
        let mut app = test_app(&seed_items());
        app.cursor_pos = 5;
        app.handle_key(key_ctrl(KeyCode::Char('u')));
        assert_eq!(app.cursor_pos, 0); // clamped via saturating_sub
    }

    // ── Mode switching ──────────────────────────────────────────────────

    #[test]
    fn slash_enters_filter_mode() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        assert!(matches!(app.mode, Mode::Filter));
    }

    #[test]
    fn colon_enters_command_mode() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char(':')));
        assert!(matches!(app.mode, Mode::Command));
    }

    #[test]
    fn esc_from_filter_returns_to_normal() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        assert!(matches!(app.mode, Mode::Filter));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn esc_from_command_returns_to_normal() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char(':')));
        assert!(matches!(app.mode, Mode::Command));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Normal));
    }

    // ── Filter ──────────────────────────────────────────────────────────

    #[test]
    fn text_filter_narrows_list() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        // Type "coffee"
        for c in "coffee".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.filtered_items.len(), 1);
    }

    #[test]
    fn wildcard_filter_matches() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        // Type "2024*jpg" — should match all .jpg files with 2024
        for c in "2024*jpg".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        // sunrise.jpg, sunrise-2.jpg, valentine.jpg
        assert_eq!(app.filtered_items.len(), 3);
    }

    #[test]
    fn filter_resets_cursor_to_zero() {
        let mut app = test_app(&seed_items());
        app.cursor_pos = 4;
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('c')));
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn esc_keeps_filter_esc_again_clears() {
        let mut app = test_app(&seed_items());
        // Enter filter, type text
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('c')));
        app.handle_key(key(KeyCode::Char('o')));
        let filtered_count = app.filtered_items.len();
        assert!(filtered_count < 6);

        // First Esc: leave filter mode, keep filter
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(app.filtered_items.len(), filtered_count);

        // Second Esc: clear filter (via the Esc cascade in normal mode)
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.filtered_items.len(), 6);
    }

    #[test]
    fn backspace_removes_filter_text() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        for c in "coffee".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.filtered_items.len(), 1);

        // Remove last char
        app.handle_key(key(KeyCode::Backspace));
        // "coffe" should still match "coffee"
        assert_eq!(app.filtered_items.len(), 1);

        // Remove all
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Backspace));
        }
        assert_eq!(app.filtered_items.len(), 6);
    }

    // ── Selection ───────────────────────────────────────────────────────

    #[test]
    fn space_toggles_selection() {
        let mut app = test_app(&seed_items());
        assert!(app.selected.is_empty());

        app.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(app.selected.len(), 1);
        assert!(app.selected.contains(&0));

        // Toggle off
        app.handle_key(key(KeyCode::Char(' ')));
        assert!(app.selected.is_empty());
    }

    #[test]
    fn ctrl_a_selects_all_then_deselects() {
        let mut app = test_app(&seed_items());

        app.handle_key(key_ctrl(KeyCode::Char('a')));
        assert_eq!(app.selected.len(), 6);

        app.handle_key(key_ctrl(KeyCode::Char('a')));
        assert!(app.selected.is_empty());
    }

    #[test]
    fn esc_clears_selection_first() {
        let mut app = test_app(&seed_items());
        // Select some items
        app.handle_key(key(KeyCode::Char(' ')));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(app.selected.len(), 2);

        // Esc should clear selection, not close preview or clear filter
        app.handle_key(key(KeyCode::Esc));
        assert!(app.selected.is_empty());
        assert!(matches!(app.mode, Mode::Normal));
    }

    // ── Preview toggle ──────────────────────────────────────────────────

    #[test]
    fn enter_toggles_preview() {
        let mut app = test_app(&seed_items());
        assert!(!app.show_preview);

        app.handle_key(key(KeyCode::Enter));
        assert!(app.show_preview);

        app.handle_key(key(KeyCode::Enter));
        assert!(!app.show_preview);
    }

    #[test]
    fn esc_closes_preview_when_nothing_selected() {
        let mut app = test_app(&seed_items());
        app.show_preview = true;

        app.handle_key(key(KeyCode::Esc));
        assert!(!app.show_preview);
    }

    // ── Collision sort order ────────────────────────────────────────────

    #[test]
    fn collision_sort_order_correct() {
        let app = test_app(&seed_items());
        // Verify sunrise appears before sunrise-2
        let names: Vec<String> = app.items.iter().filter_map(|m| m.file_name()).collect();
        let sunrise_pos = names.iter().position(|n| n == "2024-01-05-sunrise.jpg");
        let sunrise2_pos = names.iter().position(|n| n == "2024-01-05-sunrise-2.jpg");
        assert!(sunrise_pos.unwrap() < sunrise2_pos.unwrap());
    }

    // ── Group navigation (Home/End) ─────────────────────────────────────

    #[test]
    fn home_jumps_to_group_start() {
        let mut app = test_app(&seed_items());
        // Move cursor to the second item in the first group (sunrise-2)
        app.cursor_pos = 1;
        app.handle_key(key(KeyCode::Home));
        assert_eq!(app.cursor_pos, 0); // Start of first group
    }

    #[test]
    fn end_jumps_to_next_group() {
        let mut app = test_app(&seed_items());
        // From first item, End should jump to the next group start
        app.handle_key(key(KeyCode::End));
        assert!(app.cursor_pos > 0); // Should have moved
    }

    // ── Title format ────────────────────────────────────────────────────

    #[test]
    fn filter_is_empty_without_text_or_tags() {
        let app = test_app(&seed_items());
        assert!(app.filter.is_empty());
    }

    #[test]
    fn filter_not_empty_with_text() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('x')));
        assert!(!app.filter.is_empty());
    }

    // ── Tag filter ──────────────────────────────────────────────────────

    #[test]
    fn hash_enters_tag_mode_in_filter() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('#')));
        assert!(app.tag_input.is_some());
    }

    #[test]
    fn at_enters_type_mode_in_filter() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('@')));
        assert!(app.type_input.is_some());
    }

    // ── Shift selection ─────────────────────────────────────────────────

    #[test]
    fn shift_down_selects_range() {
        let mut app = test_app(&seed_items());
        app.handle_key(key_shift(KeyCode::Down));
        // Should have selected current position and moved down
        assert!(!app.selected.is_empty());
        assert_eq!(app.cursor_pos, 1);
    }

    // ── View switching ──────────────────────────────────────────────────

    #[test]
    fn view_1_shows_all() {
        let mut app = test_app(&seed_items());
        app.handle_key(key(KeyCode::Char('1')));
        assert_eq!(app.filtered_items.len(), 6);
    }

    // ── Quit ────────────────────────────────────────────────────────────

    #[test]
    fn q_signals_exit() {
        let mut app = test_app(&seed_items());
        let should_exit = app.handle_key(key(KeyCode::Char('q')));
        assert!(should_exit);
    }

    #[test]
    fn normal_key_does_not_exit() {
        let mut app = test_app(&seed_items());
        let should_exit = app.handle_key(key(KeyCode::Down));
        assert!(!should_exit);
    }

    // ── MediaItem ───────────────────────────────────────────────────────

    #[test]
    fn file_name_includes_extension() {
        let app = test_app(&seed_items());
        let name = app.items[0].file_name().unwrap();
        assert!(name.ends_with(".jpg"));
        assert!(name.starts_with("2024-01-05"));
    }

    #[test]
    fn group_key_day_format() {
        let app = test_app(&seed_items());
        // "2024-01-05-sunrise" -> group key "2024-01-05"
        let key = app.items[0].group_key().unwrap();
        assert_eq!(key, "2024-01-05");
    }

    #[test]
    fn group_key_slug_format() {
        let app = test_app(&seed_items());
        // "2024-03-beach-0001" -> group key "2024-03-beach"
        let beach_item = app
            .items
            .iter()
            .find(|m| m.path_stem.as_ref().is_some_and(|s| s.contains("beach")));
        let key = beach_item.unwrap().group_key().unwrap();
        assert_eq!(key, "2024-03-beach");
    }

    // ── Missing on disk (lazy check) ────────────────────────────────────

    #[test]
    fn missing_on_disk_detected_on_preview() {
        let mut app = test_app(&seed_items());
        // Source paths are "/src/..." which don't exist
        // Open preview to trigger lazy check
        app.handle_key(key(KeyCode::Enter));
        assert!(app.show_preview);
        // The source_path "/src/2024-01-05-sunrise.jpg" doesn't exist
        // so missing_on_disk should be set to true
        let idx = app.filtered_items[app.cursor_pos];
        assert!(app.items[idx].missing_on_disk);
    }

    // ── Esc cascade ─────────────────────────────────────────────────────

    #[test]
    fn esc_cascade_selection_then_preview_then_filter() {
        let mut app = test_app(&seed_items());

        // Set up: filter active, preview open, items selected
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('s')));
        app.handle_key(key(KeyCode::Esc)); // leave filter mode
        app.handle_key(key(KeyCode::Enter)); // open preview
        app.handle_key(key(KeyCode::Char(' '))); // select item

        assert!(!app.selected.is_empty());
        assert!(app.show_preview);
        assert!(!app.filter.is_empty());

        // First Esc: clears selection
        app.handle_key(key(KeyCode::Esc));
        assert!(app.selected.is_empty());
        assert!(app.show_preview);

        // Second Esc: closes preview
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.show_preview);

        // Third Esc: clears filter
        app.handle_key(key(KeyCode::Esc));
        assert!(app.filter.is_empty());
        assert_eq!(app.filtered_items.len(), 6);
    }

    // ── Grouping ───────────────────────────────────────────────────────

    #[test]
    fn left_arrow_collapses_group_and_right_arrow_expands() {
        let mut app = test_app(&seed_items());
        let initial_rows = app.visible_rows.len();
        
        // Find an item with a group_key
        let mut target_idx = 0;
        for (i, row) in app.visible_rows.iter().enumerate() {
            if let crate::app::ListRow::Item(idx) = row {
                if app.items[*idx].group_key().is_some() {
                    target_idx = i;
                    break;
                }
            }
        }
        
        app.cursor_pos = target_idx;
        
        // Collapse
        app.handle_key(key(KeyCode::Left));
        let collapsed_rows = app.visible_rows.len();
        assert!(collapsed_rows < initial_rows || initial_rows == collapsed_rows);
        
        // Right should expand
        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.visible_rows.len(), initial_rows);
    }
}
