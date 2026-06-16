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
    self,
    client::*,
    calls::*,
};
// use jf_import_library::api::{self, ApiCall, ApiCallFuture, ApiManifest, QueryStatus, TMDBClient, error::QueryError};
use jf_import_library::media::{
    MediaCatalog, 
    types::MediaItem, 
    tree::{TreeGenError, TreeNode}
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
    catalog: Rc<MediaCatalog>,

    media_list: Vec<MediaItem>,
    view_state: RefCell<ListState>,
    
    api_manifest: ApiManifest,
    api_client: Arc<dyn api::client::ApiClient + Send + Sync>,
}
impl CatalogView {
    pub fn new(catalog: Rc<MediaCatalog>) -> Result<Self, TreeViewError> {
        // let state = if catalog.movies.len()+catalog.shows.len() > 0 {
        //        ListState::default().with_selected(Some(0)) }
        // else { ListState::default() };
        let mut state = ListState::default();
        if catalog.movies.len()+catalog.shows.len() > 0 { state = state.with_selected(Some(0)); }
        tracing::info!("Total media items while creating view: {}", catalog.movies.len()+catalog.shows.len());
        tracing::info!("New State: {:?}", state.selected());

        let mut view = CatalogView {
            catalog,

            media_list: Vec::new(),
            view_state: RefCell::new(state),

            api_client: Arc::new(TMDBClient::new()),
            api_manifest: ApiManifest::new(),
        };

        view.stage_api_calls();
        Ok(view)
    }
    pub async fn update(&mut self) -> Result<(), TreeViewUpdateError> {
        if self.catalog.movies.len()+self.catalog.shows.len() != self.media_list.len() {
            let movies = self.catalog.movies.iter()
                .map(|m| MediaItem::Movie(m.clone()));
            let shows = self.catalog.shows.iter()
                .map(|s| MediaItem::Show(s.clone()));
            self.media_list = movies.chain(shows).collect()
        }
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
        for (idx, movie) in self.catalog.movies.iter().enumerate() {
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
        for (idx, show) in self.catalog.shows.iter().enumerate() {
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
    fn select_next(&self) {
        let mut lock = self.view_state.borrow_mut();
        match lock.selected() {
            Some(index) if index < self.media_list.len()-1 => { lock.select_next(); }
            None => { lock.select(Some(0)); }
            Some(_) => {}
        }
    }
    fn select_previous(&self) {
        let mut lock = self.view_state.borrow_mut();
        match lock.selected() {
            Some(index) if index >= 1 => { lock.select_previous(); }
            None => { lock.select(Some(0)); }
            Some(_) => {}
        }
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
        let frame_border = Block::new()
            .bold()
            .fg(Color::Rgb(153, 121, 61))
            .border_type(BorderType::Rounded)
            .title_bottom(instructions.clone().centered());

        frame.render_widget(&frame_border, frame.area());

        let outer_layout = Layout::horizontal([
            Constraint::Length(32),
            Constraint::Fill(1),
            Constraint::Ratio(1, 3)])
            .spacing(Spacing::Overlap(1))
            // .margin(1)
            .split(frame.area());

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy);

        let lock = self.view_state.borrow();
        let items = MediaList::from_iter(self.media_list
            .iter()
            .map(|media| {
                // this shouldn't fail, we don't do multithreading and we 
                // don't hold the lock across `await`s
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
            }))
            .set_state(lock.clone())
        ;
        items.render(outer_layout[1], frame.buffer_mut());

        let mut lock = match BRAILLE.try_lock() {
            Ok(lock) => lock,
            Err(err) => return,
        };

        // Filler until I decide how to populate this empty space
        Paragraph::new("").block(block.clone())
            .render(outer_layout[0], frame.buffer_mut());
        Paragraph::new::<String>((&*lock).into()).block(block.clone())
            .render(outer_layout[2], frame.buffer_mut());
        drop(lock);
    }
    fn handle_input(self, code: crossterm::event::KeyCode) {
        // let mut lock = self.view_state.borrow_mut();
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
                for k in self.media_list.iter() {
                    lock.insert(k.clone(), Rc::new(QueryStatus::InProgress));
                }
            }
            _ => {}
        }
    }
}
