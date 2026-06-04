use ratatui::{self,crossterm};

pub mod miller_columns;
pub mod tree_view;
pub mod generating_view;

pub use miller_columns::MillerColumn;
pub use tree_view::TreeView;
pub use generating_view::GeneratingView;

pub mod error {
    pub use super::tree_view::TreeViewError;
    pub use super::generating_view::GeneratingViewError;
}

pub trait Renderable {
    fn render(&self, frame: &mut ratatui::Frame);
    fn handle_input(&mut self, key: crossterm::event::KeyCode);
}
