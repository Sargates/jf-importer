use std::cell::RefCell;

use ratatui::{
    buffer::Buffer, 
    layout::*, 
    style::{Color,Stylize}, 
    text::Line, 
    widgets::*, 
    *
};

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::select;

use jf_import_library::api::{client::QueryStatus, error::QueryError};
use jf_import_library::media::{
    Catalog, CatalogBuilder, CatalogBuildError,
};
use jf_import_library::media;
use jf_import_library::config::*;

use crate::widgets::{BrailleLoadingIcon, Renderable};

#[derive(Debug)]
pub enum GeneratingWidgetError {
    PanicOnBuilderThread(tokio::task::JoinError),
    CatalogGenError(CatalogBuildError),
    Timeout,
    RecvError(watch::error::RecvError)
}

pub struct GeneratingWidget {
    thread_handle: JoinHandle<Result<Catalog, CatalogBuildError>>,
    completed: Option<Catalog>,

    buffer: RefCell<MessageDisplay>
}
impl GeneratingWidget {
    pub fn new() -> Self {
        let builder = CatalogBuilder::new(CONFIG.clone());
        let mut buffer = MessageDisplay::new(builder.subscribe());

        // this subscriber needs to be properly handled after we call `builder.build`
        let thread_handle = tokio::task::spawn_blocking(move || {
            tracing::info!("Creating GeneratingView");
            let res = builder.build();
            match &res {
                Ok(_)  => tracing::info!("[GenerationThread] Successfully generated catalog"),
                Err(e) => tracing::info!("[GenerationThread] Failed to generate catalog: {e:?}"),
            }
            res
        });

        Self {
            thread_handle,
            completed: None,
            buffer: RefCell::new(buffer)
        }
    }
    pub async fn poll(&mut self) -> Result<(), GeneratingWidgetError> {
        let mut borrow_mut = self.buffer.borrow_mut();
        select! {
            res = &mut self.thread_handle => {
                match res {
                    Ok(owned) => {
                        tracing::info!("Finished!!!");
                        self.completed = Some(owned.map_err(|e| GeneratingWidgetError::CatalogGenError(e))?);
                    }
                    Err(e) => {
                        tracing::error!("Failed to join thread while generating media catalog! Error: {:?}", e);
                        Err(e).map_err(|e| GeneratingWidgetError::PanicOnBuilderThread(e))?
                    }
                }
            }
            res = borrow_mut.poll() => {
                match res {
                    Ok(_) => {}
                    Err(err) => { Err(err)? }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                tracing::info!("Timed out waiting for GeneratingView subscriber!!");
                Err(GeneratingWidgetError::Timeout)?
            }
        }
        Ok(())
    }
    pub fn is_complete(&self) -> bool { self.thread_handle.is_finished() && self.completed.is_some() }
    pub fn take(&mut self) -> Option<Catalog> { self.completed.take() }
}

struct MessageDisplay {
    pub inner: Option<Buffer>,
    pub last: (String, String),
    pub subscriber: watch::Receiver<String>,
}
impl MessageDisplay {
    pub fn new(subscriber: watch::Receiver<String>) -> Self {
        Self {
            inner: None,
            last: ("Unset Previous".into(), "Unset".into()),
            subscriber,
        }
    }
    pub fn generate(&mut self, rect: Rect)
        { self.inner = Some(Buffer::empty(rect)) }
    pub fn is_unset(&self) -> bool
        { self.inner.is_none() }
    pub fn set(&mut self, inner: Buffer)
        { self.inner = Some(inner) }
    pub fn take(&mut self) -> Option<Buffer>
        { self.inner.take() }
    pub async fn poll(&mut self) -> Result<(), GeneratingWidgetError> {
        select! {
            res = self.subscriber.changed() => {
                match res {
                    Ok(()) => {
                        let message = self.subscriber.borrow_and_update().to_string();
                        tracing::info!("[] Message Received from : {}", message);
                        self.last=(self.last.1.clone(), message); 
                    }
                    Err(e) => {
                        tracing::info!("[] Error when receiving from channel: {:?}", e);
                        Err(e).map_err(|e| GeneratingWidgetError::RecvError(e))?;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                tracing::info!("Timed out waiting for GeneratingView subscriber!!");
                Err(GeneratingWidgetError::Timeout)?
            }
        }
        Ok(())
    }
}

impl WidgetRef for GeneratingWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized {
        let mut borrow_mut = self.buffer.borrow_mut();
        borrow_mut.render(area, buf);
    }
}

impl Widget for &mut MessageDisplay {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized {

        // let mut borrow_mut = self.inner.borrow_mut();
        let mut buffer = match self.inner.take() {
            Some(buffer) => buffer,
            None => { Buffer::empty(area) }
        };

        // paths are unique, so messages from the MediaCatalogBuilder are too. this is 
        // just to prevent messy output that a user may notice due to slower crawling
        if self.last.0 == self.last.1 { return; }
        
        let popup_block = Block::bordered()
            // .border_type(BorderType::Rounded)
            .title(" Generating Media Catalog ")
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy)
            .fg(Color::Rgb(153, 121, 61))
        ;
        let mut new_inner = {
            // we take the lines `1..N`, dropping the 0th line, and pad the end of the buffer so it's the right size.
            // ratatui doesn't expose any helpful way to intersect two buffers. `Buffer::merge` just unions two 
            // of them together and exposes no other methods of boolean-ing them together, so there's no way 
            // to be clever about doing this
            let no_border = popup_block.inner(buffer.area);
            // let no_border = buffer.area;
            let mut dummy = buffer.clone();
            dummy.resize(no_border);
            let mut iter = dummy.content.into_iter();
            let mut out = Buffer::empty(no_border);

            let size = no_border.clone().as_size();
            let position = no_border.clone().as_size();
            out.content = iter
                .skip(size.width.into())
                .collect();
            out.content.extend(Buffer::empty(Rect::new(1,1, size.width, 1)).content);
            out
        };
        // assert_eq!(popup_block.inner(buffer.area), new_inner.area);

        Widget::render(Clear, buffer.area, buf);
        let dummy = Paragraph::new("")
            .bold()
            // .fg(Color::Green)
            .block(popup_block.clone());
        Widget::render(dummy, buffer.area, buf);

        let paragraph = Paragraph::new(format!("{}", self.last.1))
            .bold()
            // .fg(Color::Green)
        ;
        let size = new_inner.area.clone().as_size();
        let pos = new_inner.area.clone().as_position();
        let write_area = Rect::new(pos.x, pos.y+size.height-1, size.width, 1); // last line of buffer
        Widget::render(&paragraph, write_area, &mut new_inner);            // render paragraph to last line of buffer
        // tracing::info!("new_inner:\n{:?}", new_inner);
        buf.merge(&new_inner);
        Widget::render(&paragraph, write_area, buf);

        // need to re-create a buffer of the original size so that we 
        // don't recursively make the buffer smaller and smaller
        new_inner.resize(buffer.area().clone());
        self.inner = Some(new_inner);
    }
}
