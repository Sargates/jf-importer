use std::cell::RefCell;
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

use jf_import_library::media::types::MediaItem;
use jf_import_library::api::client::{QueryResponse, QueryStatus};

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
        self.state = RefCell::new(ListState::default());
        self.inner = new;
    }
    pub fn set_state(mut self, state: ListState) -> Self {
        *self.state.borrow_mut() = state;
        self
    }
}

impl FromIterator<MediaListItem> for MediaList {
    /// You may want to call `Default::default` instead
    fn from_iter<T: IntoIterator<Item = MediaListItem>>(iter: T) -> Self {
        Self {
            inner: iter.into_iter().collect(),
            state: RefCell::new(ListState::default()),
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
                    QueryStatus::Success(response) => {
                        response.title.clone()
                    },
                    _ => {
                        match &item.inner {
                            MediaItem::Movie(movie) => {
                                movie.src.file_stem().unwrap().to_string_lossy().to_string()
                            }
                            MediaItem::Show(show) => {
                                show.src.file_name().unwrap().to_string_lossy().to_string()
                            }
                            MediaItem::Episode(ep)   => {
                                ep.id.to_string()
                            }
                        }
                    }
                };

                // TODO: make these tests
                // if debug {
                //     tracing::info!("[PADDING] Name:           {}", name);
                //     tracing::info!("[PADDING] Bytes:          {:?}", name.bytes());
                //     tracing::info!("[PADDING] Label:          {}", label);
                //     tracing::info!("[PADDING] Name Len:       {}", name.len());
                //     tracing::info!("[PADDING] Name Max:       {}", name_max);
                //     tracing::info!("[PADDING] Label Len:      {}", label.len());
                //     tracing::info!("[PADDING] Padding Len:    {}", padding.len());
                //     // tracing::info!("[PADDING] Calculated:     {}", out);
                //     tracing::info!("[PADDING] Calculated Len: {}", out.len());
                // }
                // // assert_eq!(out.len(), width);

                Line::from(name).left_aligned().into()
            })
            .collect()
        ;
        let statuses: Vec<ListItem> = self.inner
            .iter()
            .map(|item| {
                let lock = BRAILLE.try_lock().expect("BRAILLE was locked when drawing MediaList");
                let label: String = match &*item.status {
                    QueryStatus::NotStarted => "[Not Started]".into(),
                    QueryStatus::InProgress => (&*lock).into(),
                    QueryStatus::Failed(query_error) => "[ ✗ ]".into(),
                    QueryStatus::Success(query_response) => "[ ✓ ]".into(),
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

// impl Into<ListItem> for MediaListItem {
//     fn into(self) -> ListItem {
//         let mut src: Span = match &self.inner {
//             MediaItem::Movie(inner)   => inner.src.to_string_lossy().into(),
//             MediaItem::Show(inner)    => inner.src.to_string_lossy().into(),
//             MediaItem::Episode(inner) => inner.src.to_string_lossy().into(),
//         };
//         let tag: Span = match self.status {
//             QueryStatus::Success(query) => "".into(), // no tag
//             QueryStatus::Failed(err)    => "[Failure]".into(),
//             QueryStatus::NotStarted     => "[NotStarted]".into(),
//             QueryStatus::InProgress     => "[InProgress]".into()
//         };
//         match self.status {
//             QueryStatus::Success(query) => {
//                 src = format!("{} ({}) [imdb-{}]", query.title, query.year, query.tmdb).into();
//             },
//             _ => {}
//         }
//         let [title, tag_area] = Layout::horizontal(vec![
//             Constraint::Fill(1),
//             Constraint::Length(tag.to_string().len() as u16),
//         ]).areas(self.area);
//         let line = Line::from(vec![
//             src,
//             " ".repeat(self.area.width - ),
//             tag
//         ]);
//         let mut buffer = Buffer::empty(self.area);
//         Widget::render(src, title,    &mut buffer);
//         Widget::render(tag, tag_area, &mut buffer);
//
//         ListItem::from(Text::from(buffer.content))
//     }
// }

