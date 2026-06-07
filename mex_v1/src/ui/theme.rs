use ratatui::style::Color;

// Theme configuration for consistency across the app
pub const COLOR_TEXT: Color = Color::White;
pub const COLOR_DIM: Color = Color::DarkGray;
pub const COLOR_SLUG: Color = Color::Cyan;
pub const COLOR_CAPTION: Color = Color::Yellow;
pub const COLOR_HIGHLIGHT_BG: Color = Color::Rgb(50, 50, 90);
pub const COLOR_MISSING_FG: Color = Color::Rgb(220, 100, 100);
pub const COLOR_MISSING_BG: Color = Color::Rgb(60, 15, 15);
pub const COLOR_FILTER_MATCH_BG: Color = Color::Rgb(100, 100, 150);
#[allow(dead_code)]
pub const COLOR_SUCCESS: Color = Color::Green;
#[allow(dead_code)]
pub const COLOR_ERROR: Color = Color::Red;
