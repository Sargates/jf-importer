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
    Catalog, CatalogBuilder,
    MediaItem, 
};
use jf_import_library::config::CONFIG;

use unicode_segmentation::UnicodeSegmentation;

use crate::widgets::*;

pub enum TreeViewUpdateError {

}

#[derive(Debug)]
pub enum TreeViewError {
    EmptyTree
}

#[derive(Default)]
pub enum ActiveInfoWidget {
    #[default]
    None,
    GeneratingCatalog(GeneratingWidget)
}
pub enum InfoWidgetError {
    NoError,
    GeneratingCatalog(GeneratingWidgetError)
}

#[derive(Default)]
pub struct CatalogView {
    catalog: Option<Catalog>,
    
    api_client: Option<Arc<dyn api::client::ApiClient + Send + Sync>>,
    api_manifest: ApiManifest,

    // info_widget: ActiveInfoWidget, // can't get this working right now
    info_widget: Option<GeneratingWidget>,

    media_list: MediaList,
}

impl CatalogView {
    pub fn with_client(mut self, client: Arc<dyn api::client::ApiClient+Sync+Send>) -> Self {
        self.api_client = Some(client);
        self.stage_api_calls();
        self
    }
    pub fn with_catalog(mut self, catalog: Catalog) -> Self {
        self.catalog = Some(catalog);
        self.update_media_list();
        self
    }
    pub async fn update(&mut self) -> Result<(), TreeViewUpdateError> {
        select! {
            Some(res) = self.api_manifest.next(), if !self.api_manifest.is_empty() => {
                self.process_api_result(res);
            }
            status = async { self.info_widget.as_mut().unwrap().poll().await }, if self.info_widget.is_some() => {
                match status {
                    Ok(_) => { self.take_catalog(); },
                    Err(err) => { tracing::info!("Error generating config: {:?}", err); },
                }
            }
            _ = futures::future::ready(()) => {} // non-blocking await
        }

        match BRAILLE.try_lock() {
            Ok(mut lock) => lock.tick_next(),
            Err(err) => tracing::error!("Failed to acquire lock for ticking braille widget: Err: {}", err),
        };
        Ok(())
    }
    pub fn take_catalog(&mut self) -> Result<(), TreeViewUpdateError> {
        if let Some(generator) = &mut self.info_widget && generator.is_complete() {
            tracing::info!("[CatalogView::post_update] Generation complete");
            let generator = generator.take();
            tracing::info!("[CatalogView::post_update] Here1");
            if let Some(catalog) = &generator {
                tracing::info!("[CatalogView::post_update] Catalog Generated");
                self.set_catalog(generator);
                self.stage_api_calls(); // re-stage API calls for outgoing
            }
            tracing::info!("[CatalogView::post_update] Here2");
            self.info_widget = None;
        }
        Ok(())
    }
    fn stage_api_calls(&mut self) {
        if let Some(catalog) = &self.catalog && let Some(client) = &self.api_client {
            for media in catalog.iter_all() {
                let future = ApiCallFuture::new(media.clone(), client.clone());
                self.api_manifest.push_future(future);
                // this shouldn't fail, we don't do multithreading and we 
                // don't hold the lock across `await`s
                let mut lock = self.api_manifest.try_lock().unwrap(); 
                let status = Rc::new(QueryStatus::NotStarted);
                lock.insert(media.clone(), status.clone());
            }
        }
        else { tracing::warn!("Tried to stage api calls while `self.catalog = Option::None`") }
    }
    fn update_media_list(&mut self) {
        if let Some(catalog) = &self.catalog {
            self.media_list.update(catalog.iter_all()
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
        else { tracing::warn!("Tried to update media list while `self.catalog = Option::None`") }
    }
    fn select_next(&mut self)     { self.media_list.select_next(); }
    fn select_previous(&mut self) { self.media_list.select_previous(); }

    pub fn get_client(&self) -> Option<Arc<dyn api::client::ApiClient+Sync+Send>> { self.api_client.clone() }
    pub fn set_client(&mut self, client: Option<Arc<dyn api::client::ApiClient+Sync+Send>>) { self.api_client = client; self.stage_api_calls(); }
    pub fn get_catalog(&self) -> Option<&Catalog>   { self.catalog.as_ref() }
    pub fn set_catalog(&mut self, catalog: Option<Catalog>) {
        if let Some(catalog) = &catalog {
               tracing::info!("Total media items while creating view: {}", catalog.iter_all().count()); } 
        else { tracing::info!("Creating `CatalogView` with no catalog"); }
        self.catalog = catalog;
        self.update_media_list();
    }

    pub fn generate_catalog(&mut self) {
        let generator = GeneratingWidget::new();
        // self.info_widget = ActiveInfoWidget::GeneratingCatalog(generator);
        self.info_widget = Some(generator);
    }
    pub fn process_api_result(&mut self, result: ApiCall) {
        match &result.status {
            QueryStatus::Success(response) => tracing::info!(
                "[Success] Result: src -> {:?}",
                format!("{} ({}) [tmdbid-{}]", response.title, response.year, response.tmdb)),
            QueryStatus::Failed(query_error) => tracing::error!(
                "[Failure] Failed to query API: {:?}",
                query_error),
            _ => {}
        }
        let mut lock = self.api_manifest.try_lock().unwrap();
        lock.insert(result.item, Rc::new(result.status));
        drop(lock);

        // Update the media list
        self.update_media_list();
    }
    // async fn poll_info_widget(&mut self) -> Result<(), InfoWidgetError> {
    //     match &mut self.info_widget {
    //         ActiveInfoWidget::None => { Ok(()) },
    //         ActiveInfoWidget::GeneratingCatalog(generating_widget) => {
    //             generating_widget.poll().await
    //                 .map_err(|e| InfoWidgetError::GeneratingCatalog(e))
    //         },
    //     }
    // }
}

// TODO: do this properly
// `into` and `from` are fucking annoying, create a separate file for all the 
// "sub-widgets" we want to display in a `CatalogView`. single-line, fold, media info, fold info
//? What did I mean by "fold"?
//?     as in folding episodes within a show?
//?     Maybe duplicate media files too?

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
        // Paragraph::new("").block(styled_block.clone())
        //     .render(user_info, frame.buffer_mut());

        // let info_rect = styled_block.inner(user_info);
        let info_rect = user_info;
        match &self.info_widget {
            Some(generator) => { generator.render_ref(info_rect, frame.buffer_mut()); }
            None => { Clear.render(info_rect, frame.buffer_mut()); }
        }

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
                if let Some(catalog) = &self.catalog {
                    tracing::info!("Sending API requests");
                    self.api_manifest.send();
                    let mut lock = self.api_manifest.try_lock().unwrap();
                    for k in catalog.iter_all() {
                        lock.insert(k.clone(), Rc::new(QueryStatus::InProgress));
                    }
                    drop(lock);

                    // update self.media_list with update hashmap values
                    self.update_media_list();
                } 
                else { self.generate_catalog(); }
            }
            _ => {}
        }
    }
}
