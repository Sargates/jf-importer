use std::{rc::Rc, cell::RefCell};
use jf_import_library::{api::QueryResponse, media_item::MediaItem};
use jf_import_library::media_catalog::TreeNode;

use std::collections::LinkedList;

use crossterm::event::KeyCode;
use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Style,Stylize,Modifier}
};
use ratatui::style::palette::tailwind::{BLUE, GREEN, SLATE};

pub struct MillerColumn {
    inner: Vec<TreeNode>,
    state: ListState,
}
impl MillerColumn {
    pub fn new(inner: Vec<TreeNode>) -> MillerColumn {
        let mut state = ListState::default();
        if inner.len() > 0 { state = state.with_selected(Some(0)); }
        MillerColumn {
            inner,
            state
        }
    }
    pub fn get_selected(&self) -> TreeNode { self.inner[self.get_selected_index()].clone() }
    pub fn get_selected_index(&self) -> usize { self.state.selected().unwrap() }
    pub fn select_next(&mut self) {
        if self.state.selected().unwrap() < self.inner.len()-1 { self.state.select_next(); } }
    pub fn select_prev(&mut self) { 
        if self.state.selected().unwrap() > 0 { self.state.select_previous(); } }
}

pub struct MillerColumns {
    prev: LinkedList<MillerColumn>,
    curr: MillerColumn,
    next: Option<MillerColumn>,
}
impl MillerColumns {
    pub fn new(starting_column: MillerColumn) -> MillerColumns {
        let mut out = MillerColumns {
            prev: LinkedList::new(),
            curr: starting_column,
            next: None
        };
        out.post_update();
        out
    }

    /// Generates the next miller column based on the column at `self.curr` 
    /// and the selected index in `curr`
    pub fn generate_next(&self) -> Option<MillerColumn> {
        let opt = self.curr
            .get_selected()
            .children()
            .and_then(|r| Some(r.clone()));
        if opt.is_none() 
            { return None; }
        let children = opt.unwrap();
        if children.is_empty() 
            { return None }
        Some(MillerColumn::new(children))
    }

    pub fn select_next(&mut self) { self.curr.select_next(); self.post_update(); }
    pub fn select_prev(&mut self) { self.curr.select_prev(); self.post_update(); }
    pub fn count(&self) -> usize { self.prev.len() + 1usize + Into::<usize>::into(self.next.is_some()) }
    
    pub fn step_into(&mut self) {
        // check if next is valid
        if let Some(next) = self.next.take() {
               // avoid cloning
               self.prev.push_back(std::mem::replace(&mut self.curr, next)); } 
        else { return; } // don't update if it wouldn't do anything

        // set up next
        self.post_update();
    }
    pub fn step_out(&mut self) {
        // check if next is valid
        if let Some(prev) = self.prev.pop_back() {
            // avoid cloning
            self.next = Some(std::mem::replace(&mut self.curr, prev));
        } 
    }
    /// Used to update `self.next` after performing `self.select_next`, `self.select_prev`, or
    /// `self.step_into`
    pub fn post_update(&mut self) {
        self.next = self.generate_next();
    }
}

impl Widget for &mut MillerColumn {
    fn render(self, area: Rect, buf: &mut prelude::Buffer)
    where 
        Self: Sized {
        let block = Block::new()
            .borders(Borders::all())
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy)
            // .border_set(symbols::border::EMPTY)
            // .border_style(Style::new().fg(SLATE.c100).bg(BLUE.c800))
            ;

        let items: Vec<ListItem> = self
            .inner
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let node = node.clone();
                match node {
                    TreeNode::Category{ name, .. } => {
                        ListItem::from(name.clone())
                    }
                    TreeNode::Item { inner, .. } => {
                        ListItem::from(match inner {
                            MediaItem::Movie(movie) => {
                                match movie.try_lock() {
                                    Ok(guard) => {
                                        match &guard.query {
                                            Some(response) => response.title.to_string(),
                                            None           => guard.src.file_stem().unwrap().to_string_lossy().to_string(),
                                        }
                                    }
                                    Err(err) => format!("Unknown Movie (failed to get lock)")

                                }
                            }
                            MediaItem::Show(show) => {
                                match show.try_lock() {
                                    Ok(guard) => {
                                        match &guard.query {
                                            Some(response) => response.title.to_string(),
                                            None           => guard.src.file_name().unwrap().to_string_lossy().to_string(),
                                        }
                                    }
                                    Err(err) => format!("Unknown Show (failed to get lock)")
                                }
                            }
                            MediaItem::Episode(ep)   => {
                                match ep.try_lock() {
                                    Ok(guard) => {
                                        match &guard.query {
                                            Some(response) => response.title.to_string(),
                                            None           => guard.id.to_string().clone()
                                        }
                                    }
                                    Err(err) => format!("Unknown Episode (failed to get lock)")

                                }
                            }
                        }
                        )
                    }
                    TreeNode::Fail{ buf, .. } => {
                        ListItem::from(buf.to_string_lossy().to_string())
                    }
                }
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ")
            .highlight_spacing(HighlightSpacing::Always);
        
        // StatefulWidget::render(list, area, buf, &mut self.state)
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}
impl Widget for &mut MillerColumns {
    fn render(self, area: Rect, buf: &mut prelude::Buffer)
    where
        Self: Sized {
        let outer_layout = Layout::horizontal([Constraint::Ratio(1, 3); 3])
            .direction(layout::Direction::Horizontal)
            .spacing(Spacing::Overlap(1))
            // .margin(1)
            .split(*buf.area());

        if let Some(prev) = self.prev.back_mut() {
            Widget::render(prev, outer_layout[0], buf);
        } else {
            let block = Block::new()
                .borders(Borders::ALL)
                .merge_borders(symbols::merge::MergeStrategy::Fuzzy);
            Widget::render(block, outer_layout[0], buf);
        }
        Widget::render(&mut self.curr, outer_layout[1], buf);
        if let Some(prev) = &mut self.next {
            Widget::render(prev, outer_layout[2], buf);
        } else {
            let block = Block::new()
                .borders(Borders::all())
                .merge_borders(symbols::merge::MergeStrategy::Fuzzy);
            Widget::render(block, outer_layout[2], buf);
        }

    }
}

