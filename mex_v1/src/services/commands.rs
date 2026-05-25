use crate::app::App;
use crate::domain::media::MediaItem;

pub fn execute(app: &mut App, cmd: &str) {
    let mut parts = cmd.split_whitespace();
    let command = parts.next().unwrap_or("");

    match command {
        "q" | "quit" => {
            std::process::exit(0);
        }
        "tag" => {
            let tag = parts.next().unwrap_or("");
            if !tag.is_empty() {
                app.status_message = Some(format!("Tag '{}' applied (mock)", tag));
                // TODO: Update DB and item tags_packed
            }
        }
        "untag" => {
            let tag = parts.next().unwrap_or("");
            if !tag.is_empty() {
                app.status_message = Some(format!("Tag '{}' removed (mock)", tag));
            }
        }
        "slugify" => {
            let slug = parts.next().unwrap_or("");
            app.status_message = Some(format!("Slugified with '{}' (mock)", slug));
        }
        "deslugify" => {
            app.status_message = Some("Deslugified (mock)".to_string());
        }
        "fix-ext" => {
            app.status_message = Some("Extensions fixed (mock)".to_string());
        }
        "fix-date" => {
            app.status_message = Some("Dates fixed (mock)".to_string());
        }
        "empty-trash" => {
            app.status_message = Some("Trash emptied (mock)".to_string());
        }
        _ => {
            app.status_message = Some(format!("Unknown command: {}", cmd));
        }
    }
}
