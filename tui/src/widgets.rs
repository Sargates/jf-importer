use futures::FutureExt;
use ratatui::{self,crossterm};

// pub mod miller_columns;
pub mod catalog_view;
pub mod generating_view;
pub mod media_list;
pub mod utils;

// pub use miller_columns::MillerColumn;
pub use catalog_view::CatalogView;
pub use generating_view::*;
pub use media_list::*;
pub use utils::*;

pub mod error {
    pub use super::catalog_view::TreeViewError;
    pub use super::generating_view::GeneratingWidgetError;
}

pub trait Renderable {
    fn render(self, frame: &mut ratatui::Frame);
    fn handle_input(self, key: crossterm::event::KeyCode);
}
