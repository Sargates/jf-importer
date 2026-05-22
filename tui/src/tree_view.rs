use jf_import_library::api_query::{QueryError, Queryable};
use jf_import_library::catalog_tree::{TreeGenError, TreeNode};
use jf_import_library::{catalog_tree, dir_search};

mod miller_columns;

use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::Color
};

use crossterm::event::KeyCode;
use crate::app::Renderable;

use self::miller_columns::MillerColumn;

fn recursive_print(node: &catalog_tree::TreeNode, old_indent: String) {
    // `tree` ripoff
    const connector: &'static str = "│   ";
    const middle:    &'static str = "├── ";
    const end:       &'static str = "└── ";
    const empty:     &'static str = "    ";

    print!("{}", old_indent);
    println!("{node}");

    let mut next_indent = if old_indent.chars().count() > 3 {
        let split_point = old_indent.char_indices().rev().nth(3).map_or(0, |(idx, _)| idx);
        let (rest, last) = old_indent.split_at(split_point);
        
        match &last.chars().nth(0).unwrap() {
            '├' => String::from(rest) + connector,
            '└' => String::from(rest) + empty,
             _  => unreachable!()
        }
    } else { String::new() };

    match &node {
        catalog_tree::TreeNode::Show {show, children} => {
            for child in children {
                let mut copy = next_indent.clone();
                let last = children.last().unwrap();
                // ref: https://users.rust-lang.org/t/is-any-way-to-know-references-are-referencing-the-same-object/9716/6
                if child as *const _ != children.last().unwrap() as *const _ 
                     { copy += middle; }
                else { copy += end; }
                recursive_print(child, copy.clone());
            }
        }
        catalog_tree::TreeNode::Category { name, children } => {
            if name == "Failures" { return; }
            for child in children {
                let mut copy = next_indent.clone();
                let last = children.last().unwrap();
                // ref: https://users.rust-lang.org/t/is-any-way-to-know-references-are-referencing-the-same-object/9716/6
                if child as *const _ != children.last().unwrap() as *const _ 
                     { copy += middle; }
                else { copy += end; }
                recursive_print(child, copy.clone());
            }
        }
        _ => {}
    }
}

pub struct TreeView {
    tree: catalog_tree::CatalogTree,
    columns: Vec<MillerColumn>
}
impl TreeView {
    pub fn new() -> Result<Self, TreeGenError> {
        let tree = dir_search::generate_catalog_tree()?;
        let mut view = TreeView{tree, columns: vec![]};
        view.update_column();
        Ok(view)
    }
    pub fn update_column(&mut self) {
        let mut vec = vec![];

        //* Testing. Select Movies Category
        let categories = self.tree.tree.children();
        let movies_cat = categories.get(0).unwrap();
        let movies = movies_cat.children();
        //* End Testing

        for movie in movies {
            if let TreeNode::Movie(movie) = movie {
                vec.push(TreeNode::Movie(movie.clone()));
            }
        }

        if self.columns.len() > 0 {
            drop(self.columns.pop());
        }
        self.columns.push(miller_columns::MillerColumn::new(vec));
    }
}

impl Renderable for TreeView {
    fn render(&mut self, frame: &mut ratatui::Frame) {
        let outer_layout = Layout::default()
            .direction(layout::Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Ratio(1, 3); 3])
            .split(frame.area());

        frame.render_widget(self.columns.get_mut(0).unwrap(), outer_layout[1]);
    }
    fn handle_input(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Char('j') => {
                self.columns[0].next();
            }
            crossterm::event::KeyCode::Char('k') => {
                self.columns[0].prev();
            }
            _ => {}
        }
    }
}
