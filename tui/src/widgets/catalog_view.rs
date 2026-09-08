use ratatui::{
    *,
    layout::*,
    widgets::*,
    buffer::Buffer,
    crossterm::event::{KeyEvent, KeyCode},
    style::{Color, Modifier, Style, Styled, Stylize},
    text::{Text, Line, Span, ToSpan},
};
use ratatui_macros::{text, line};
use ratatui::style::palette::tailwind::{BLUE, GREEN, SLATE};

use futures::{future::{self, Pending}, stream::{FuturesUnordered, StreamExt}};
use tokio::task::JoinHandle;
use tokio::select;

use std::{default, ops::Deref, rc::Rc};
use std::cell::RefCell;
use std::pin::Pin;
use std::sync::Arc;

use jfi::{
    api::{ self, calls::*, client::* },
    catalog::{Catalog, CatalogBuilder},
    config::Config,
    media::{MediaItem, ffprobe::FFprobeFailure} 
};

use unicode_segmentation::UnicodeSegmentation;

use crate::{ffprobe::ProbeResult, widgets::*};
use crate::config::*;
use crate::ratatui_ext::*;

pub enum TreeViewUpdateError {

}

#[derive(Debug)]
pub enum TreeViewError {
    EmptyTree
}
pub enum InfoWidgetError {
    NoError,
    GeneratingCatalog(GeneratingWidgetError),
    MediaInfoWidget(FFprobeFailure),
}

#[derive(Default)]
pub enum InfoWidget {
    /// Denotes an inactive space, do nothing
    #[default]
    Inactive,

    /// Displays messages from the CatalogBuilder thread.
    Generating(GeneratingWidget),

    /// Since it's dynamic, this is more of a "signal" to generate a MediaInfoWidget
    MediaInfo,
}

pub struct CatalogView {
    config: Arc<Config>,
    catalog: Option<Catalog>,

    // info_widget: ActiveInfoWidget, // can't get this working right now
    info_widget: InfoWidget,

    media_list: MediaList,

    prober: crate::ffprobe::Prober,
}

impl CatalogView {
    pub fn new(config: Arc<jfi::Config>) -> Self {
        Self {
            config,
            catalog: None,
            info_widget: InfoWidget::Inactive,
            media_list: MediaList::default(),
            prober: crate::ffprobe::Prober::default(),
        }
    }
    pub fn with_catalog(mut self, catalog: Catalog) -> Self {
        self.catalog = Some(catalog);
        self.update_media_list();
        self
    }
    pub async fn update(&mut self) -> Result<(), TreeViewUpdateError> {
        // make an intermediate future to check conditions
        // that require making multiple method calls
        let info_status = async {
            // if the info widget is invalid, `select!` should never
            // resolve this branch, so return pending
            match self.info_widget {
                InfoWidget::Inactive => {
                    futures::future::pending::<Option<InfoWidgetError>>().await
                },
                InfoWidget::Generating(ref mut widget) => {
                    widget.poll().await
                        .map_err(InfoWidgetError::GeneratingCatalog)
                        .err()
                },
                InfoWidget::MediaInfo => {
                    // this is blocking, synchronous work, relatively inexpensive.
                    // doesn't really matter if this blocks; this isn't embedded
                    self.prober.update();
                    futures::future::pending::<Option<InfoWidgetError>>().await
                },
            }
        };
        let poll_apis = async {
            // if the info widget is invalid, `select!` should never
            // resolve this branch, so return pending
            if let Some(ref mut catalog) = self.catalog {
                let mut lock = catalog.try_lock_manifest().unwrap();
                lock.next().await
            }
            else { future::pending().await }
        };

        select! {
            Some(res) = poll_apis => {
                if let Some(ref mut catalog) = self.catalog &&
                   let Ok(mut manifest) = catalog.try_lock_manifest() {
                    manifest.update_api_call(res);
                }
                if let Some(ref catalog) = self.catalog {
                    self.update_media_list();
                }
            }
            status = info_status => {}
            _ = futures::future::ready(()) => {} // non-blocking await
        }

        {
            let state = std::mem::replace(&mut self.info_widget, InfoWidget::Inactive);
            self.info_widget = match state {
                InfoWidget::Inactive => { InfoWidget::Inactive },
                InfoWidget::Generating(mut widget) => {
                    if self.ingest_if_complete(&mut widget) {
                        InfoWidget::MediaInfo
                    } else {
                        InfoWidget::Generating(widget)
                    }
                },
                InfoWidget::MediaInfo => { InfoWidget::MediaInfo },
            };
        }

        Ok(())
    }
    pub fn ingest_if_complete(&mut self, widget: &mut GeneratingWidget) -> bool {
        if !widget.is_some() { return false; }

        // tracing::info!("Generating widget is completed. Extracting result");

        let catalog: Option<Catalog> = match widget.take() {
            Some(Ok(c)) => Some(c),
            Some(Err(e)) => {
                tracing::info!("[ingest_if_complete] Error while generating catalog: {:?}", e);
                None
            },
            _ => {
                panic!("`GeneratingWidget::is_completed` returned true while `GeneratingWidget::widget.take` returned `None`");
            }
        };

        self.set_catalog(catalog);

        return true;
    }

    pub fn set_catalog(&mut self, catalog: Option<Catalog>) {
        self.catalog = catalog;
        let count = if let Some(ref catalog) = self.catalog {
            Some(catalog.iter_all().count())
        } else { None };

        if let Some(ref mut catalog) = self.catalog {
            tracing::info!("Total media items while creating view: {}", count.unwrap());

            catalog.stage_api_calls();
        }
        else { tracing::info!("Creating `CatalogView` with no catalog"); }
        self.update_media_list();
    }

    fn update_media_list(&mut self) {
        if let Some(ref mut catalog) = self.catalog {
            self.media_list.update_list(catalog.iter_all_sorted()
                .map(|media| {
                    let lock = catalog.try_lock_manifest().unwrap();
                    let status = match lock.get(&media) {
                        Some(arc) => arc.clone(),
                        None => Arc::new(QueryStatus::NotStarted),
                    };
                    drop(lock);
                    MediaListItem {
                        inner: media.clone(),
                        status,
                    }
                })
                .collect());
            if let Some(selected) = self.media_list.get_selected() &&
               let None = self.prober.get(&selected) {
                self.prober.dispatch(selected);
            }
        }
        // else if let Some(Err(e)) = &self.catalog {
        //     tracing::warn!("Tried to update media list while `self.catalog` failed to generate. Error: {e:?}") }
        else { tracing::warn!("Tried to update media list while `self.catalog: None`") }
    }

    pub fn generate_catalog(&mut self) {
        let generator = GeneratingWidget::new(self.config.clone());
        self.info_widget = InfoWidget::Generating(generator);
    }

    pub fn draw_catalog_info(&self, area: Rect, buf: &mut Buffer) {
        if let Some(ref catalog) = self.catalog &&
           let Ok(lock) = catalog.try_lock_manifest()
        {
            // We filter out non-successes and non-failures,
            // then partition based on those. Then we get two
            // dedicated vectors.
            // I can't figure out how to keep them as iterators
            // to avoid re-allocation clone `collect`ing.
            // Maybe `itertools` can help?
            let (successes, failures): (Vec<_>, Vec<_>) = catalog.iter_all()
                .map(|m| lock.get(&m))
                .filter_map(|a| a)
                .filter(|a| match ***a {
                    QueryStatus::Success(_) | QueryStatus::Failed(_) => true,
                    _ => false
                })
                .partition(|a| match ***a {
                    QueryStatus::Success(_) => true,
                    QueryStatus::Failed(_)  => false,
                    _ => unreachable!(),
                })
            ;

            let successes = successes.into_iter().count();
            let fails = failures.into_iter().count();
            let total = catalog.iter_all().count();

            text![
                format!("Successs: {successes}/{total}")
                    .set_style_if(successes == total, Style::new().green()),
                format!("Failures: {fails}/{total}")
                    .set_style_if(fails > 0, Style::new().red()),
            ]
                .render(area, buf);
        }
    }

    // ************************** LAYOUT **************************
    fn layout_generate(&self, area: Rect) -> [Rect; 4] {
        let [left, list_view, split] = Layout::horizontal([
            Constraint::Length(32),
            Constraint::Fill(1),
            Constraint::Ratio(1, 3)])
            .spacing(Spacing::Overlap(1))
            // .margin(1)
            .areas(area);
        let [query_info, user_and_media_info] = Layout::vertical([
            Constraint::Fill(3),
            Constraint::Fill(2)])
            .spacing(Spacing::Overlap(1))
            .areas(split);
        [left, list_view, query_info, user_and_media_info]
    }
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

        let instructions = Block::new()
            .bold()
            .fg(Color::Rgb(153, 121, 61))
            .border_type(BorderType::Rounded)
            .title_bottom(instructions.centered());
        frame.render_widget(&instructions, frame.area());

        let [left, list_view, query_info, user_and_media_info] = self.layout_generate(frame.area());

        let styled_block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy)
        ;

        // Filler until I decide how to populate this empty space
        Paragraph::new("").block(styled_block.clone())
            .render(left, frame.buffer_mut());
        Paragraph::new("").block(styled_block.clone())
            .render(list_view, frame.buffer_mut());
        Paragraph::new("").block(styled_block.clone())
            .render(query_info, frame.buffer_mut());
        Paragraph::new("").block(styled_block.clone())
            .render(user_and_media_info, frame.buffer_mut());

        let map = |r| styled_block.inner(r);
        let left = map(left);
        let list_view = map(list_view);
        let query_info = map(query_info);

        self.media_list.render(list_view, frame.buffer_mut());

        let info_block = styled_block.clone()
            .title_top(Line::from(" Information ".add_modifier(Modifier::REVERSED).bold()).right_aligned())
        ;
        Paragraph::new("")
            .block(info_block.clone())
            .render(user_and_media_info, frame.buffer_mut());

        self.draw_catalog_info(left, frame.buffer_mut());

        // TODO: move these branches to separate method calls
        //       signature:
        //       `render_generating_view(w: &mut GeneratingWidget)`
        //       (maybe) `render_media_info(c: &mut Catalog, lock: MutexLock<T>, selected: MediaItem)`
        match &mut self.info_widget {
            InfoWidget::Inactive => {},
            InfoWidget::Generating(widget) => {
                widget.block(info_block);
                widget.render_ref(user_and_media_info, frame.buffer_mut());
            },
            InfoWidget::MediaInfo => {
                if let Some(ref mut catalog) = self.catalog &&
                   let Ok(lock) = catalog.try_lock_manifest() &&
                   let Some(item) = self.media_list.get_selected()
                {
                    // transition rect inner of border
                    let user_and_media_info = map(user_and_media_info);

                    let res = self.prober.get(&item);
                    match &res {
                        Some(ProbeResult::Success(info)) => {
                            let widget = MediaInfoWidget::new(item, info);
                            widget.render(user_and_media_info, frame.buffer_mut());
                        }
                        Some(ProbeResult::Failure(err)) => {
                            let message = match err {
                                FFprobeFailure::FFprobeExecutableNotFound => format!("Failed to locate `ffprobe` executable"),
                                FFprobeFailure::FailedToSpawnProcess      => format!("Failed to spawn sub-process"),
                                FFprobeFailure::InputDoesNotExist         => format!("`ffprobe` input does not exist"),
                                FFprobeFailure::InputIsNotAFile           => format!("`ffprobe` input is not a file"),
                                FFprobeFailure::NoJsonOutput              => format!("No JSON output from `ffprobe` command (internal error)"),
                                FFprobeFailure::FailedToParseJson(error)  => format!("Failed to parse JSON output from `ffprobe` (internal error)"),
                            };
                            Paragraph::new(message).render(user_and_media_info, frame.buffer_mut());
                        }
                        _ => {
                            // iff the item isn't probed yet and an item is selected, dispatch
                            if let None = res &&
                            let Some(selected) = self.media_list.get_selected() {
                                self.prober.dispatch(selected);
                            }
                            // show braille icon with paragraph
                            if let Some(icon) = BRAILLE.try_lock().ok() {
                                let icon: String = icon.deref().into();
                                let message: Span<'_> = format!("Loading probe: {}", icon).into();
                                Paragraph::new(message).render(user_and_media_info, frame.buffer_mut());
                            }
                        }
                    }
                }
            },
        }
    }
    fn handle_input(self, event: crossterm::event::KeyEvent) {
        // tracing::info!("Key code: {:?}        Key Modifier: {:?}", event.code, event.modifiers);
        // TODO: move this update somewhere else.
        // there needs to be a separate synchronous and async update loop
        let ctrl_pressed = event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL);
        match event.code {
            crossterm::event::KeyCode::Char('g') if ctrl_pressed => { self.media_list.goto_top(); }
            crossterm::event::KeyCode::Char('G')                 => { self.media_list.goto_bottom(); }
            crossterm::event::KeyCode::Char('d') if ctrl_pressed => { self.media_list.half_down(); }
            crossterm::event::KeyCode::Char('u') if ctrl_pressed => { self.media_list.half_up(); }
            crossterm::event::KeyCode::Char('j') => { self.media_list.select_next(); }
            crossterm::event::KeyCode::Char('k') => { self.media_list.select_previous(); }


            crossterm::event::KeyCode::Char('o') => {
                if let Some(ref c) = self.catalog {
                    let c1: Vec<String> = c.iter_all().map(|m| format!("{:?}", m))
                        .filter(|m| m.contains("Enron: The Smartest Guys in the Room.mkv"))
                        .collect();
                    tracing::info!("All: {:#?}", c1);
                    let c2: Vec<String> = c.iter_all_sorted().map(|m| format!("{:?}", m))
                        .filter(|m| m.contains("Enron: The Smartest Guys in the Room.mkv"))
                        .collect();
                    tracing::info!("Sorted: {:#?}", c2);
                }
            }

            // TODO: move this to a traversible menu
            crossterm::event::KeyCode::Char('p') => {
                match self.info_widget {
                    InfoWidget::Inactive => {
                        self.generate_catalog();
                    },
                    InfoWidget::Generating(_) => {}
                    InfoWidget::MediaInfo if let Some(ref mut catalog) = self.catalog => {
                        let mut lock = catalog.try_lock_manifest().unwrap();
                        tracing::info!("Sending API requests");
                        lock.send();
                        for k in catalog.iter_all() {
                            let call_status = ApiCall {
                                item: k.clone(), status: QueryStatus::InProgress
                            };
                            lock.update_api_call(call_status);
                        }
                        drop(lock);
                        // update self.media_list with update hashmap values
                        self.update_media_list();
                    },
                    InfoWidget::MediaInfo => {},
                }
            }
            _ => {

                // tracing::info!("Invalid Key Combindation!");
            }
        }
        if let Some(selected) = self.media_list.get_selected() &&
           let None = self.prober.get(&selected) {
            self.prober.dispatch(selected);
        }
    }
}
