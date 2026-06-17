use ratatui::{
    *,
    layout::*,
    widgets::*,
    buffer::Buffer, 
    crossterm::event::KeyCode,
    style::{Color, Modifier, Style, Stylize},
    text::{Text, Line, Span, ToSpan}, 
};
use ratatui::style::palette::tailwind::{BLUE, GREEN, SLATE};

use futures::stream::{StreamExt, FuturesUnordered};
use tokio::task::JoinHandle;
use tokio::select;

use std::{default, rc::Rc};
use std::cell::RefCell;
use std::pin::Pin;
use std::sync::Arc;

use jf_import_library::api::{
    self, calls::*, client::*
};
// use jf_import_library::api::{self, ApiCall, ApiCallFuture, ApiManifest, QueryStatus, TMDBClient, error::QueryError};
use jf_import_library::media::{
    Catalog, 
    MediaItem, 
};

use unicode_segmentation::UnicodeSegmentation;

use crate::widgets::*;
use super::Renderable;

pub enum TreeViewUpdateError {

}

#[derive(Debug)]
pub enum TreeViewError {
    EmptyTree
}
pub struct CatalogView {
    catalog: Catalog,
    
    api_manifest: ApiManifest,
    api_client: Arc<dyn api::client::ApiClient + Send + Sync>,

    media_list: MediaList,
}
impl CatalogView {
    pub fn new(catalog: Catalog) -> Result<Self, TreeViewError> {
        tracing::info!("Total media items while creating view: {}", catalog.iter_all().count());

        let mut view = CatalogView {
            catalog,
            api_client: Arc::new(TMDBClient::new()),
            api_manifest: ApiManifest::new(),

            media_list: MediaList::default(),
        };

        view.stage_api_calls();
        view.update_media_list();

        Ok(view)
    }
    pub async fn update(&mut self) -> Result<(), TreeViewUpdateError> {
        let media_list_update: Option<Vec<MediaListItem>> = None;
        select! {
            Some(result) = self.api_manifest.next(), if !self.api_manifest.is_empty() => {
                match &result.status {
                    QueryStatus::Success(response) => tracing::info!(
                        "[Success] Result: src -> {:?}",
                        format!("{} ({}) [tmdbid-{}]", response.title, response.year, response.tmdb)
                    ),
                    QueryStatus::Failed(query_error) => tracing::error!(
                        "[Failure] Failed to query API: {:?}",
                        query_error
                    ),
                    _ => {}
                }
                let mut lock = self.api_manifest.lock().await;
                lock.insert(result.item, Rc::new(result.status));
                drop(lock);
                

                // Update the media list
                self.update_media_list();
            }
            _ = futures::future::ready(()) => {} // non-blocking await
        }

        match BRAILLE.try_lock() {
            Ok(mut lock) => lock.tick_next(),
            Err(err) => tracing::error!("Failed to acquire lock for ticking: Err: {}", err),
        };
        Ok(())
    }
    fn stage_api_calls(&mut self) {
        // TODO: remove these redundant `enumerate` calls
        for movie in self.catalog.iter_movies() {
            let movie = MediaItem::Movie(movie.clone());
            let client = self.api_client.clone();
            let future = ApiCallFuture::new(movie.clone(), client);
            self.api_manifest.push_future(future);
            // this shouldn't fail, we don't do multithreading and we 
            // don't hold the lock across `await`s
            let mut lock = self.api_manifest.try_lock().unwrap(); 
            let status = Rc::new(QueryStatus::NotStarted);
            lock.insert(movie, status.clone());
        }

        // TODO: remove these redundant `enumerate` calls
        for show in self.catalog.iter_shows() {
            let show = MediaItem::Show(show.clone());
            let client = self.api_client.clone();
            let future = ApiCallFuture::new(show.clone(), client);
            self.api_manifest.push_future(future);
            // this shouldn't fail, we don't do multithreading and we 
            // don't hold the lock across `await`s
            let mut lock = self.api_manifest.try_lock().unwrap(); 
            let status = Rc::new(QueryStatus::NotStarted);
            lock.insert(show, status.clone());
        }
    }

    fn update_media_list(&mut self) {
        self.media_list.update(self.catalog.iter_all()
            .map(|media| {
                let lock = self.api_manifest.try_lock().unwrap();
                let status = match lock.get(&media) {
                    Some(rc) => rc.clone(),
                    None => Rc::new(QueryStatus::NotStarted),
                };
                drop(lock);
                MediaListItem {
                    inner: media.clone(),
                    status,
                }
            })
            .collect());
    }
    fn select_next(&mut self) {
        self.media_list.select_next();
    }
    fn select_previous(&mut self) {
        self.media_list.select_previous();
    }
}

// TODO: do this properly
// `into` and `from` are fucking annoying, create a separate file for all the 
// "sub-widgets" we want to display in a `CatalogView`. single-line, fold, media info, fold info
//? What did I mean by "fold"? as in folding episodes within a show?

impl Renderable for &mut CatalogView {
    fn render(self, frame: &mut ratatui::Frame) {
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
            " Make API Calls ".into(),
            format!("<{}>", KeyCode::Char('p')).blue().bold(),
            " ".into(),
        ]);

        // We don't draw the border here
        let instructions = Block::new()
            .bold()
            .fg(Color::Rgb(153, 121, 61))
            .border_type(BorderType::Rounded)
            .title_bottom(instructions.centered());
        frame.render_widget(&instructions, frame.area());

        let [left, list_view, split] = Layout::horizontal([
            Constraint::Length(32),
            Constraint::Fill(1),
            Constraint::Ratio(1, 3)])
            .spacing(Spacing::Overlap(1))
            // .margin(1)
            .areas(frame.area());
        let [media_info, user_info] = Layout::vertical([
            Constraint::Fill(3),
            Constraint::Fill(2)])
            .spacing(Spacing::Overlap(1))
            .areas(split);
        // [left, list_view, media_info, user_info]

        let styled_block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy);

        self.media_list.render(list_view, frame.buffer_mut());

        // Filler until I decide how to populate this empty space
        Paragraph::new("").block(styled_block.clone())
            .render(left, frame.buffer_mut());
        Paragraph::new("").block(styled_block.clone())
            .render(media_info, frame.buffer_mut());
        Paragraph::new("").block(styled_block.clone())
            .render(user_info, frame.buffer_mut());

        // {
        //     let mut lock = match BRAILLE.try_lock() {
        //         Ok(lock) => lock,
        //         Err(err) => return,
        //     };
        //     Paragraph::new::<String>((&*lock).into()).block(styled_block.clone())
        //         .render(media_info, frame.buffer_mut());
        //     drop(lock);
        // }

    }
    fn handle_input(self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Char('j') => {
                self.select_next();
            }
            crossterm::event::KeyCode::Char('k') => {
                self.select_previous();
            }
            crossterm::event::KeyCode::Char('l') => {}
            crossterm::event::KeyCode::Char('h') => {}
            crossterm::event::KeyCode::Char('p') => {
                self.api_manifest.send();
                let mut lock = self.api_manifest.try_lock().unwrap();
                for k in self.catalog.iter_all() {
                    lock.insert(k.clone(), Rc::new(QueryStatus::InProgress));
                }
                drop(lock);
                self.update_media_list();
            }
            _ => {}
        }
    }
}
