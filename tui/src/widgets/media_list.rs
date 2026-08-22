use std::{cell::RefCell, vec};
use std::rc::Rc;

use ratatui::{
    *,
    layout::*,
    widgets::*,
    buffer::Buffer, 
    style::{Color, Modifier, Style, Stylize},
    text::{Text, Line, Span, ToSpan}, 
};
use ratatui::style::palette::tailwind::{BLUE, GREEN, SLATE};

use unicode_segmentation::UnicodeSegmentation;

use jfi::media::MediaItem;
use jfi::api::client::{QueryResponse, QueryStatus};

use crate::widgets::BRAILLE;


pub struct MediaListItem {
    pub inner: MediaItem,
    pub status: Rc<QueryStatus>,
}

#[derive(Default)]
pub struct MediaList {
    inner: Vec<MediaListItem>,
    state: RefCell<ListState>,
}
impl MediaList {
    pub fn update(&mut self, new: Vec<MediaListItem>) {
        let mut state = ListState::default();
        if new.len() > 0 { state = state.with_selected(Some(0)); } // autoselect first element
        let mut borrow = self.state.borrow_mut();
        let opt = borrow.selected();
        if let Some(i) = opt && i < new.len() {
            borrow.select(Some(i)); 
        } else if let None = opt && new.len() > 0 {
            borrow.select(Some(0)); 
        }
        self.inner = new;
    }
    pub fn set_state(mut self, state: ListState) -> Self {
        *self.state.borrow_mut() = state;
        self
    }
    pub fn select_next(&mut self) {
        let mut lock = self.state.borrow_mut();
        match lock.selected() {
            Some(index) if index < self.inner.len()-1 => { lock.select_next(); }
            None => { lock.select(Some(0)); }
            Some(_) => {}
        }
    }
    pub fn select_previous(&mut self) {
        let mut lock = self.state.borrow_mut();
        match lock.selected() {
            Some(index) if index >= 1 => { lock.select_previous(); }
            None => { lock.select(Some(0)); }
            Some(_) => {}
        }
    }
}

impl Widget for &MediaList {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized {
        let items: Vec<ListItem> = self.inner
            .iter()
            .map(|item| {
                let name = match &*item.status {
                    QueryStatus::Success(response) => { response.title.clone() },
                    _ => { item.inner.raw_media_label() }
                };
                Line::from(name).left_aligned().into()
            })
            .collect()
        ;
        let statuses: Vec<ListItem> = self.inner
            .iter()
            .map(|item| {
                let lock = BRAILLE.try_lock().expect("BRAILLE was locked when drawing MediaList");
                let label: Span<'_> = match &*item.status {
                    QueryStatus::NotStarted => "[Not Started]".into(),
                    QueryStatus::InProgress => (&*lock).into(),
                    QueryStatus::Failed(query_error) => "[ ✗ ]".red(),
                    QueryStatus::Success(query_response) => "[ ✓ ]".green(),
                };
                Line::from(label).right_aligned().into()
            })
            .collect();

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy);

        // TODO: make this styling global.
        // Something similar to this: https://github.com/LucasPickering/slumber/blob/master/crates/tui/src/view/styles.rs
        let left_title = block.clone()
            .title_alignment(Alignment::Left)
            .title(" Media Item ".add_modifier(Modifier::REVERSED).bold());
        let dummy = Paragraph::new("")
            .block(left_title);
        Widget::render(dummy, area, buf);
        let right_title = block.clone()
            .title_alignment(Alignment::Right)
            .title(" Api Call Status ".add_modifier(Modifier::REVERSED).bold());
        let list = List::default()
            .block(right_title)
            .highlight_style(Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD))
            // .highlight_symbol("> ") // there's no easy way to add padding like this and size the whole widget programatically based on it
            .highlight_spacing(HighlightSpacing::Always)
            .scroll_padding(10)
        ;

        let names = list.items(items);
        let mut lock = self.state.borrow_mut();
        StatefulWidget::render(&names, area, buf, &mut lock);
        let statuses = names.items(statuses);
        StatefulWidget::render(&statuses, area, buf, &mut lock);
    }
}

