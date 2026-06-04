use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Stylize}
};

use std::rc::Rc;
use std::cell::RefCell;

use jf_import_library::api::QueryError;
use jf_import_library::media_catalog::{MediaCatalog, TreeGenError, TreeNode};
use jf_import_library::media_item::MediaItem;
use jf_import_library::media_catalog;

use crossterm::event::KeyCode;
use super::Renderable;

use super::miller_columns::*;


#[derive(Debug)]
pub enum TreeViewError {
    EmptyTree
}
pub struct TreeView {
    tree: Rc<MediaCatalog>,
    columns: RefCell<MillerColumns>,
    column_depth: usize,
}
impl TreeView {
    pub fn new(tree: Rc<MediaCatalog>) -> Result<Self, TreeViewError> {
        let column = MillerColumn::new(tree.tree.children().unwrap().clone());
        let mut view = TreeView{tree, columns: RefCell::new(MillerColumns::new(column)), column_depth: 0};
        Ok(view)
    }
}

impl Renderable for TreeView {
    fn render(&self, frame: &mut ratatui::Frame) {
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
        let mut borrow_mut = self.columns.borrow_mut();
        frame.render_widget(&mut *borrow_mut, frame.area());
    }
    fn handle_input(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Char('j') => {
                self.columns.borrow_mut().select_next();
            }
            crossterm::event::KeyCode::Char('k') => {
                self.columns.borrow_mut().select_prev();
            }
            crossterm::event::KeyCode::Char('l') => {
                self.columns.borrow_mut().step_into();
            }
            crossterm::event::KeyCode::Char('h') => {
                self.columns.borrow_mut().step_out();
            }
            _ => {}
        }
    }
}
