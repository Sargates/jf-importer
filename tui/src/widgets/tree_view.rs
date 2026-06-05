use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Stylize},
    crossterm::event::KeyCode,
};

use futures::stream::{StreamExt, FuturesUnordered};
use tokio::task::JoinHandle;
use tokio::select;

use std::rc::Rc;
use std::cell::RefCell;
use std::sync::Arc;

use jf_import_library::api::{self, QueryStatus, QueryError, TMDBClient};
use jf_import_library::media_catalog::{MediaCatalog, TreeGenError, TreeNode};
use jf_import_library::media_item::MediaItem;
use jf_import_library::global::*;

use super::Renderable;
use super::miller_columns::*;

pub enum TreeViewUpdateError {

}

#[derive(Debug)]
pub enum TreeViewError {
    EmptyTree
}
pub struct TreeView {
    tree: Rc<MediaCatalog>,
    columns: RefCell<MillerColumns>,
    column_depth: usize,
    api_client: Arc<dyn api::ApiClient + Send + Sync>,
    api_futures: FuturesUnordered<JoinHandle<MediaItem>>,
}
impl TreeView {
    pub fn new(tree: Rc<MediaCatalog>) -> Result<Self, TreeViewError> {
        let column = MillerColumn::new(tree.tree.children().unwrap().clone());
        let mut view = TreeView{
            tree,
            columns: RefCell::new(MillerColumns::new(column)),
            column_depth: 0,
            api_client: Arc::new(TMDBClient::new()),
            api_futures: FuturesUnordered::new()
        };
        Ok(view)
    }
    pub async fn update(&mut self) -> Result<(), TreeViewUpdateError> {
        if !self.api_futures.is_empty() {
            select! {
                Some(result) = self.api_futures.next() => {
                    match result {
                        Ok(returned)    => { tracing::info!("API call returned, result: {:?} -> {:?}", returned, API_CALLS.get_query(&returned).unwrap()); },
                        Err(join_error) => { tracing::error!("Failed to join future and main thread! Error: {:?}", join_error); },
                    }
                }
                timed_out = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                    tracing::info!("Timed out on awaiting an API future");
                }
            }
        }
        Ok(())
    }
    fn make_api_calls(&mut self) {
        // TODO: remove these redundant `enumerate` calls
        for (idx, movie) in self.tree.movies.iter().enumerate() {
            let movie = movie.clone();
            let client = self.api_client.clone();
            let future = tokio::task::spawn(async move {
                let status = client.search_movie(movie.clone()).await;
                let item = MediaItem::Movie(movie);
                API_CALLS.push_query(item.clone(), status);
                // let formatted_name = status.to_string(movie.src.to_string_lossy().to_string());
                // let mut lock = movie.query.lock().await;
                // *lock = status;
                // format!("Movie #{idx}: {formatted_name}")
                item
            });
            self.api_futures.push(future);
        }

        // TODO: remove these redundant `enumerate` calls
        for (idx, show) in self.tree.shows.iter().enumerate() {
            let show = show.clone();
            let client = self.api_client.clone();
            let future = tokio::task::spawn(async move {
                let status = client.search_show(show.clone()).await;
                let item = MediaItem::Show(show);
                API_CALLS.push_query(item.clone(), status);
                // let formatted_name = status.to_string(show.src.to_string_lossy().to_string());
                // let mut lock = show.query.lock().await;
                // *lock = status;
                // format!("Show #{idx}: {formatted_name}")
                item
            });
            self.api_futures.push(future);
        }
    }
}

impl Renderable for TreeView {
    fn render(&self, frame: &mut ratatui::Frame) {
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
        let block = Block::new()
            .bold()
            .fg(Color::Rgb(153, 121, 61))
            // .borders(Borders::ALL)
            .title_bottom(instructions.clone().centered());

        frame.render_widget(&block, frame.area());
        // frame.render_widget(Clear, block.inner(frame.area()));
        let mut borrow_mut = self.columns.borrow_mut();
        frame.render_widget(&mut *borrow_mut, frame.area());
    }
    fn handle_input(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Char('j') => {
                self.columns.borrow_mut().select_next();
            }
            crossterm::event::KeyCode::Char('k') => {
                self.columns.borrow_mut().select_prev();
            }
            crossterm::event::KeyCode::Char('l') => {
                self.columns.borrow_mut().step_into();
            }
            crossterm::event::KeyCode::Char('h') => {
                self.columns.borrow_mut().step_out();
            }
            crossterm::event::KeyCode::Char('p') => {
                self.make_api_calls();
            }
            _ => {}
        }
    }
}
