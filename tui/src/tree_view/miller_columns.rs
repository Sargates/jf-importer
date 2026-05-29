use std::{rc::Rc, cell::RefCell};
use jf_import_library::api_query::QueryResponse;
use jf_import_library::catalog_tree::TreeNode;

use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Style,Modifier}
};
use ratatui::style::palette::tailwind::{BLUE, GREEN, SLATE};

pub struct MillerColumn {
    inner: Vec<TreeNode>,
    state: ListState,
}
impl MillerColumn {
    pub fn new(inner: Vec<TreeNode>) -> MillerColumn {
        let mut state = ListState::default();
        if inner.len() > 0 { state.select_next(); }
        MillerColumn {
            inner,
            state
        }
    }
    pub fn next(&mut self) { self.state.select_next(); }
    pub fn prev(&mut self) { self.state.select_previous(); }
}

impl Widget for &mut MillerColumn {
    fn render(self, area: Rect, buf: &mut prelude::Buffer)
    where 
        Self: Sized {
        let block = Block::new()
            .title(Line::raw("TODO List").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(Style::new().fg(SLATE.c100).bg(BLUE.c800));

        let items: Vec<ListItem> = self
            .inner
            .iter()
            .enumerate()
            .map(|(i, node)| {
                match node {
                    TreeNode::Category{ name, .. } => {
                        ListItem::from(name.clone())
                    }
                    TreeNode::Movie(movie) => {
                        ListItem::from(
                            match &movie.borrow().query {
                                QueryResponse::Movie {title, ..} => title.to_string(),
                                QueryResponse::None => movie.borrow().src.file_stem().unwrap().to_string_lossy().to_string(),
                                _ => panic!("Invalid Movie: {}", movie.borrow().src.to_string_lossy())
                            }
                        )
                    }
                    TreeNode::Show{ show, .. } => {
                        ListItem::from(
                            match &show.borrow().query {
                                QueryResponse::Show { title, .. } => title.to_string(),
                                QueryResponse::None => show.borrow().src.file_name().unwrap().to_string_lossy().to_string(),
                                _ => panic!("Invalid Show: {}", show.borrow().src.to_string_lossy())
                            }
                        )
                    }
                    TreeNode::Episode(ep)   => {
                        ListItem::from(ep.borrow().id.to_string().clone())
                    }
                    TreeNode::Fail{ buf, .. } => {
                        ListItem::from(buf.to_string_lossy())
                    }
                }
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD))
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        
        
        // StatefulWidget::render(list, area, buf, &mut self.state)
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}
