use futures::FutureExt;
use ratatui::{self,crossterm};

// pub mod miller_columns;
pub mod catalog_view;
pub mod generating_widget;
pub mod media_list;
pub mod utils;
pub mod fallback_view;

// pub use miller_columns::MillerColumn;
pub use catalog_view::CatalogView;
pub use generating_widget::*;
pub use media_list::*;
pub use utils::*;
pub use fallback_view::*;

pub mod error {
    pub use super::catalog_view::TreeViewError;
    pub use super::generating_widget::GeneratingWidgetError;
}

pub trait Renderable {
    fn render(self, frame: &mut ratatui::Frame);
    fn handle_input(self, key: crossterm::event::KeyCode);
}
