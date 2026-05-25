use ratatui::Frame;
use crate::app::App;

pub mod layout;
pub mod file_list;
pub mod preview;
pub mod filter_bar;
pub mod status;
pub mod theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    layout::draw(f, app);
}
