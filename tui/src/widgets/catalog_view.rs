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

    poll_futures: bool,

    loading_icon: Rc<RefCell<BrailleLoadingIcon>>,
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
            poll_futures: false, // don't do something the user may not expect
            loading_icon: Default::default(),
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
        if self.poll_futures && !self.api_manifest.is_empty() {
            select! {
                Some(result) = self.api_manifest.next() => {
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
        }
        self.loading_icon.borrow_mut().tick_next();
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
            Some(index) => { lock.select_next(); }
            None => { lock.select(Some(0)); }
        }
    }
    fn select_previous(&self) {
        let mut lock = self.view_state.borrow_mut();
        match lock.selected() {
            Some(index) => { lock.select_previous(); }
            None => { lock.select(Some(0)); }
        }
    }
}

// TODO: do this properly
// `into` and `from` are fucking annoying, create a separate file for all the 
// "sub-widgets" we want to display in a `CatalogView`. single-line, fold, media info, fold info
/// we require passing a shared reference to a `MediaItem` because Rust doesn't know about 
/// that `MediaItem` is a wrapper for `Rc`. maybe there's a better way to do this, but whateveR
// fn media_to_list_item<'a>(item: &'a MediaItem, status: Option<Rc<QueryStatus>>, width: usize, debug: bool) -> ListItem<'a> {
// }

struct CatalogListItem {
    item: MediaItem,
    status: Option<Rc<QueryStatus>>,
    width: usize,
    loading: Rc<RefCell<BrailleLoadingIcon>>
}
impl<'a> Into<Text<'a>> for &CatalogListItem {
    fn into(self) -> Text<'a> {
        let status = match &self.status {
            Some(status) => { status },
            None => { return Text::from(Line::default().spans(vec![Span::from(format!("Item not in List: {:?}", self.item))])); },
        };
        let name = match status.as_ref() {
            QueryStatus::Success(response) => {
                response.title.clone()
            },
            _ => {
                match &self.item {
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
        let label: String = match status.as_ref() {
            QueryStatus::NotStarted => "[Not Started]".into(),
            QueryStatus::InProgress => (&*self.loading.borrow()).into(),
            QueryStatus::Failed(query_error) => "[ ✗ ]".into(),
            QueryStatus::Success(query_response) => "[ ✓ ]".into(),
        };
        let label_len = label.graphemes(true).count();
        let name_max = if name.graphemes(true).count() < self.width-label_len {
            name.graphemes(true).count() } 
        else { self.width-label_len };
        let padding = String::from(" ").repeat(self.width-name_max-label_len);
        let out: String = name.graphemes(true).take(name_max).collect::<String>() + &padding + &label;

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
        Text::from(out)
    }
}
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

        let items: Vec<ListItem> = self.media_list
            .iter()
            .map(|media| {
                // this shouldn't fail, we don't do multithreading and we 
                // don't hold the lock across `await`s
                let lock = self.api_manifest.try_lock().unwrap();
                let status = lock.get(&media).map(|r| r.clone());
                drop(lock);

                // why doesn't the compiler throw a fit about `media` going out of scope after this function call
                //? because of the lifetime annotations on `media_to_list_item`
                (&CatalogListItem {
                    item: media.clone(),
                    status,
                    width: block.inner(outer_layout[1]).width.into(),
                    loading: self.loading_icon.clone()
                }).into()
                // media_to_list_item(media, status, block.inner(outer_layout[1]).width.into(), self.poll_futures)
            })
            .collect()
        ;

        // left title block
        let left_title = block.clone()
            .title_alignment(Alignment::Left)
            .title(" Media Item ".add_modifier(Modifier::REVERSED).bold());
        let dummy = Paragraph::new("")
            .block(left_title);
        Widget::render(dummy, outer_layout[1], frame.buffer_mut());
        let right_title = block.clone()
            .title_alignment(Alignment::Right)
            .title(" Api Call Status ".add_modifier(Modifier::REVERSED).bold());
        let list = List::new(items)
            .block(right_title)
            .highlight_style(Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD))
            // .highlight_symbol("> ") // there's no easy way to add padding like this and size the whole widget programatically based on it
            .highlight_spacing(HighlightSpacing::Always)
            .scroll_padding(10)
        ;

        // draw media list (middle column
        let mut lock = self.view_state.borrow_mut();
        StatefulWidget::render(list, outer_layout[1], frame.buffer_mut(), &mut lock);

        Paragraph::new("").block(block.clone())
            .render(outer_layout[0], frame.buffer_mut());
        Paragraph::new::<String>((&*self.loading_icon.borrow_mut()).into()).block(block.clone())
            .render(outer_layout[2], frame.buffer_mut());
        

        // frame.render_widget(&mut *borrow_mut, frame.area());
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
            crossterm::event::KeyCode::Char('l') => {
                // self.columns.borrow_mut().step_into();
            }
            crossterm::event::KeyCode::Char('h') => {
                // self.columns.borrow_mut().step_out();
            }
            crossterm::event::KeyCode::Char('p') => {
                self.poll_futures = true;
                let mut lock = self.api_manifest.try_lock().unwrap();
                for k in self.media_list.iter() {
                    lock.insert(k.clone(), Rc::new(QueryStatus::InProgress));
                }
            }
            _ => {}
        }
    }
}
