use std::sync::Arc;

use jfi::api::client::QueryResponse;
// Widget to display information extracted using ffmpeg as a widget
use jfi::catalog::MediaItem;
use jfi::media::ffprobe::*;
use ratatui::layout::Constraint;
use ratatui::style::Color;
use ratatui::style::Styled;
use ratatui::widgets::Row;

use crate::widgets::Renderable;

use ratatui::{
    widgets,
};

pub struct MediaInfoWidget<'a> {
    item: MediaItem,
    info: &'a FFprobeMediaInfo,
}

impl<'a> MediaInfoWidget<'a> {
    pub fn new(item: MediaItem , info: &'a FFprobeMediaInfo) -> Self {
        Self{ item, info }
    }
}

/// Note that this is not implemented for `&MediaInfoWidget`
/// It's intended that this is recreated every frame
impl<'a> widgets::Widget for MediaInfoWidget<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where Self: Sized
    {
        use FFprobeStream as stream;

        let mut rows: Vec<widgets::Row> = Vec::new();

        // ratatui::table doesn't support vertical headers
        let headers = vec![ "Resolution", "Languages", "Bitrate" ];
        let iter = headers.iter().next();

        // Resolution
        let mut resolution = vec![ "Resolution".to_string() ];
        let mut streams = self.info.streams.iter()
            .filter(|s| if let stream::Video(_) = s { true }
                else { false }
            );
        while resolution.len() <= 1 &&
              let Some(stream) = streams.next() {
            if let stream::Video(stream) = stream {
                resolution.push(format!("{}x{}", stream.width, stream.height));
            }
        }

        rows.push(Row::new(resolution));

        // Languages
        let mut languages = vec![ "Languages: " ];
        let mut streams = self.info.streams.iter()
            .filter(|s| if let stream::Audio(_) = s { true }
                else { false }
            );
        while languages.len() <= 5 &&
              let Some(stream) = streams.next() {
            if let stream::Audio(stream) = stream &&
               let Some(ref l) = stream.language
            {
                languages.push(l.clone().into())
            }
        }
        rows.push(Row::new(languages));

        widgets::Table::new(
            rows.into_iter()
                .enumerate()
                .map(|(i, r)| r.set_style(
                    if i % 2 == 0 {
                        ratatui::style::Style::default()
                            .bg(Color::Rgb(48, 48, 48))
                    } else {
                        ratatui::style::Style::default()
                            .bg(Color::Rgb(32, 32, 32))
                    }
                )),
            [
                Constraint::Length(12),
                Constraint::Fill(1)
            ]
        ).render(area, buf);

    }
}
