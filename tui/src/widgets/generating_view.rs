use futures::FutureExt;
use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Stylize}
};

use std::sync::Arc;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::select;

use jf_import_library::api::QueryError;
use jf_import_library::media_catalog::{MediaCatalog, TreeGenError, TreeNode};
use jf_import_library::media_item::MediaItem;
use jf_import_library::media_catalog;
use jf_import_library::config::*;

use crate::app::Renderable;
// use crate::trace_dbg;

#[derive(Debug)]
pub enum GeneratingViewError {
    GeneratingThreadPanic(tokio::task::JoinError),
    TreeGenError(TreeGenError),
    Timeout,
    RecvError(watch::error::RecvError)
}

pub struct GeneratingView {
    thread_handle: JoinHandle<Result<MediaCatalog, TreeGenError>>,
    subscriber: watch::Receiver<String>,
    last: String,
    completed: Option<MediaCatalog>,
}
impl GeneratingView {
    pub fn new() -> GeneratingView {
        let catalog = MediaCatalog::new(CONFIG.clone());
        let subscriber = catalog.subscribe();
        let thread_handle = tokio::task::spawn_blocking(move || {
            tracing::info!("Creating GeneratingView");
            let res = catalog.generate_catalog_tree();
            tracing::info!("Finished GeneratingView");
            res
        });
        GeneratingView {
            thread_handle,
            subscriber,
            last: format!("Unset"),
            completed: None,
        }
    }
    pub async fn poll(&mut self) -> Result<(), GeneratingViewError> {
        select! {
            res = &mut self.thread_handle => {
                match res {
                    Ok(owned) => {
                        tracing::info!("Finished!!!");
                        self.completed = Some(owned.map_err(|e| GeneratingViewError::TreeGenError(e))?);
                    }
                    Err(e) => {
                        tracing::error!("Failed to join thread while generating media catalog! Error: {:?}", e);
                        Err(e).map_err(|e| GeneratingViewError::GeneratingThreadPanic(e))?;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                tracing::info!("Timed out waiting for GeneratingView subscriber!!");
                Err(GeneratingViewError::Timeout)?
            }
            res = self.subscriber.changed() => {
                match res {
                    Ok(()) => {
                        let message = self.subscriber.borrow_and_update().to_string();
                        tracing::info!("[] Message Received from : {}", message);
                        self.last=message; 
                    }
                    Err(e) => {
                        tracing::info!("NOW YOU SEE ME: {:?}", e);
                        Err(e).map_err(|e| GeneratingViewError::RecvError(e))?;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn is_complete(&self) -> bool { self.completed.is_some() }
    pub fn take(&mut self) -> Option<MediaCatalog> { self.completed.take() }
}

impl Renderable for GeneratingView {
    fn render(&mut self, frame: &mut ratatui::Frame)
    where
        Self: Sized {
        
        let popup_block = Block::bordered().title(" Generating Media Catalog ");
        let float = frame.area().centered(Constraint::Percentage(60), Constraint::Percentage(20));

        Widget::render(Clear, float, frame.buffer_mut());

        let paragraph = Paragraph::new(format!("{}", self.last))
            .bold()
            .fg(Color::Green)
            .block(popup_block)
            .centered()
        ;
        Widget::render(paragraph, float, frame.buffer_mut());
    }

    fn handle_input(&mut self, key: crossterm::event::KeyCode) {}
}
