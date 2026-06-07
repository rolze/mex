use crate::app::App;
use ratatui::Frame;

pub mod file_list;
pub mod filter_bar;
pub mod layout;
pub mod preview;
pub mod status;
pub mod theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    layout::draw(f, app);
}
pub mod semantic;
