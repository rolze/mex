use crate::app::App;
use rusqlite::params;

/// Executes a command and returns `true` if the app should exit.
pub fn execute(app: &mut App, cmd: &str) -> bool {
    let mut parts = cmd.split_whitespace();
    let command = parts.next().unwrap_or("");

    let targets = app.get_target_indices();
    let _ids: Vec<String> = targets
        .iter()
        .filter_map(|&i| app.items.get(i).map(|m| m.id.clone()))
        .collect();

    if command == "q" || command == "quit" {
        return true;
    }

    if targets.is_empty() && command != "empty-trash" {
        app.status_message = Some("No items selected".to_string());
        return false;
    }

    match command {
        "tag" => {
            let tag = parts.next().unwrap_or("");
            if !tag.is_empty() {
                for &idx in &targets {
                    if let Some(media) = app.items.get_mut(idx) {
                        let mut t = media.tags_packed.replace('\x1f', " ");
                        if !t.split_whitespace().any(|x| x == tag) {
                            if !t.is_empty() {
                                t.push(' ');
                            }
                            t.push_str(tag);
                            media.tags_packed = t.replace(' ', "\x1f");
                            // Simple update DB
                            let _ = app.db_conn.execute(
                                "UPDATE media SET tags_packed = ?1 WHERE id = ?2",
                                params![media.tags_packed, media.id],
                            );
                        }
                    }
                }
                app.status_message = Some(format!("Tag '{}' applied", tag));
            }
        }
        "untag" => {
            let tag = parts.next().unwrap_or("");
            if !tag.is_empty() {
                for &idx in &targets {
                    if let Some(media) = app.items.get_mut(idx) {
                        let t = media.tags_packed.replace('\x1f', " ");
                        let new_t: Vec<&str> = t.split_whitespace().filter(|&x| x != tag).collect();
                        media.tags_packed = new_t.join("\x1f");
                        let _ = app.db_conn.execute(
                            "UPDATE media SET tags_packed = ?1 WHERE id = ?2",
                            params![media.tags_packed, media.id],
                        );
                    }
                }
                app.status_message = Some(format!("Tag '{}' removed", tag));
            }
        }
        "slugify" => {
            let slug = parts.next().unwrap_or("slug");
            for &idx in &targets {
                if let Some(media) = app.items.get_mut(idx) {
                    if let Some(stem) = &media.path_stem {
                        let mut p: Vec<&str> = stem.split('_').collect();
                        if p.len() >= 2 {
                            p[1] = slug;
                        } else {
                            p.push(slug);
                        }
                        media.path_stem = Some(p.join("_"));
                        let _ = app.db_conn.execute(
                            "UPDATE media SET path_stem = ?1 WHERE id = ?2",
                            params![media.path_stem, media.id],
                        );
                    }
                }
            }
            app.status_message = Some(format!("Slugified with '{}'", slug));
        }
        "deslugify" => {
            for &idx in &targets {
                if let Some(media) = app.items.get_mut(idx) {
                    if let Some(stem) = &media.path_stem {
                        let mut p: Vec<&str> = stem.split('_').collect();
                        if p.len() >= 2 {
                            p[1] = "";
                        }
                        media.path_stem = Some(p.join("_"));
                        let _ = app.db_conn.execute(
                            "UPDATE media SET path_stem = ?1 WHERE id = ?2",
                            params![media.path_stem, media.id],
                        );
                    }
                }
            }
            app.status_message = Some("Deslugified".to_string());
        }
        "fix-ext" => {
            for &idx in &targets {
                if let Some(media) = app.items.get_mut(idx) {
                    media.ext = media.ext.to_lowercase();
                    if media.ext == ".jpeg" {
                        media.ext = ".jpg".to_string();
                    }
                    let _ = app.db_conn.execute(
                        "UPDATE media SET ext = ?1 WHERE id = ?2",
                        params![media.ext, media.id],
                    );
                }
            }
            app.status_message = Some("Extensions fixed".to_string());
        }
        "fix-date" => {
            for &idx in &targets {
                if let Some(media) = app.items.get_mut(idx) {
                    if let Some(os) = &media.os_date {
                        media.mex_date = os.clone();
                        let _ = app.db_conn.execute(
                            "UPDATE media SET mex_date = ?1 WHERE id = ?2",
                            params![media.mex_date, media.id],
                        );
                    }
                }
            }
            app.status_message = Some("Dates fixed".to_string());
        }
        "empty-trash" => {
            let _ = app
                .db_conn
                .execute("DELETE FROM media WHERE status = 'trashed'", []);
            app.items
                .retain(|m| m.status != crate::domain::media::Status::Trashed);
            // We need to trigger a filter refresh
            app.status_message = Some("Trash emptied".to_string());
        }
        _ => {
            app.status_message = Some(format!("Unknown command: {}", cmd));
        }
    }

    // Refresh filter since we modified items and empty-trash deleted items
    app.apply_filter();

    false
}
