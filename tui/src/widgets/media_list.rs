use ratatui::layout::*;
use ratatui::text::{Text,Span,Line};
use ratatui::buffer::Buffer;
use ratatui::widgets::{Widget, ListItem};
use tokio::sync::Mutex;

use jf_import_library::media_item::MediaItem;
use jf_import_library::api::{QueryResponse, QueryStatus};


struct MediaList {}
struct MediaListItem<'a> {
    pub inner: &'a MediaItem,
    pub status: &'a QueryStatus,
    pub area: Rect,
}

// impl<'a> Into<ListItem<'a>> for MediaListItem<'a> {
//     fn into(self) -> ListItem<'a> {
//         let mut src: Span<'a> = match &self.inner {
//             MediaItem::Movie(inner)   => inner.src.to_string_lossy().into(),
//             MediaItem::Show(inner)    => inner.src.to_string_lossy().into(),
//             MediaItem::Episode(inner) => inner.src.to_string_lossy().into(),
//         };
//         let tag: Span<'a> = match self.status {
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

