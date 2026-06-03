use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Stylize}
};

use std::rc::Rc;

use jf_import_library::api::QueryError;
use jf_import_library::media_catalog::{MediaCatalog, TreeGenError, TreeNode};
use jf_import_library::media_item::MediaItem;
use jf_import_library::media_catalog;

use crossterm::event::KeyCode;
use crate::app::Renderable;

use super::miller_columns::*;


#[derive(Debug)]
pub enum TreeViewError {
    EmptyTree
}
pub struct TreeView {
    tree: Rc<MediaCatalog>,
    columns: MillerColumns,
    column_depth: usize,
}
impl TreeView {
    pub fn new(tree: Rc<MediaCatalog>) -> Result<Self, TreeViewError> {
        let column = MillerColumn::new(tree.tree.children().unwrap().clone());
        let mut view = TreeView{tree, columns: MillerColumns::new(column), column_depth: 0};
        Ok(view)
    }
}

impl Renderable for TreeView {
    fn render(&mut self, frame: &mut ratatui::Frame) {
        let instructions = Line::from(vec![
            " ".into(),
            "Select Next ".into(),
            format!("<{}>", KeyCode::Char('j')).blue().bold(),
            " Select Prev ".into(),
            format!("<{}>", KeyCode::Char('k')).blue().bold(),
            " Step Inward ".into(),
            format!("<{}>", KeyCode::Char('l')).blue().bold(),
            " Step Outward ".into(),
            format!("<{}>", KeyCode::Char('h')).blue().bold(),
            " ".into(),
        ]);
        let block = Block::new()
            .bold()
            .fg(Color::Rgb(153, 121, 61))
            // .borders(Borders::ALL)
            .title_bottom(instructions.clone().centered());

        frame.render_widget(&block, frame.area());
        // frame.render_widget(Clear, block.inner(frame.area()));
        frame.render_widget(&mut self.columns, frame.area());
    }
    fn handle_input(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Char('j') => {
                self.columns.select_next();
            }
            crossterm::event::KeyCode::Char('k') => {
                self.columns.select_prev();
            }
            crossterm::event::KeyCode::Char('l') => {
                self.columns.step_into();
            }
            crossterm::event::KeyCode::Char('h') => {
                self.columns.step_out();
            }
            _ => {}
        }
    }
}
