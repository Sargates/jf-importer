use std::cell::RefCell;
use std::sync::Arc;

use ratatui::{
    buffer::Buffer, 
    layout::*, 
    style::{Color,Stylize}, 
    text::Line, 
    widgets::*, 
    *
};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::select;

use jfi::api::{client::QueryStatus, error::QueryError};
use jfi::catalog::{ Catalog, CatalogBuilder, CatalogBuildError, };
use jfi::media;
use jfi::config::*;

use crate::widgets::{BrailleLoadingIcon, Renderable};

#[derive(Debug)]
pub enum GeneratingWidgetError {
    // PanicOnBuilderThread(tokio::task::JoinError),
    CatalogGenError(CatalogBuildError),
    Timeout,
    ChannelClosed,
    // ManuallyAborted(tokio::task::JoinError),
    // RecvError(mpsc::error::TryRecvError),
    PollAfterCompletion,
}

pub struct GeneratingWidget {
    // thread_handle: JoinHandle<Result<Catalog, CatalogBuildError>>,
    build_thread_rx: mpsc::Receiver<Result<Catalog, CatalogBuildError>>,
    completed: Option<Result<Catalog, GeneratingWidgetError>>,

    block: Option<Block<'static>>,

    display: RefCell<MessageDisplay>
}
impl GeneratingWidget {
    pub fn new(config: Arc<Config>) -> Self {
        let (status_tx, status_rx) = mpsc::channel::<Result<String, CatalogBuildError>>(1000);
        let mut builder = CatalogBuilder::new(config, Some(status_tx));
        let mut display = MessageDisplay::new(status_rx);

        // this should just be moved to a channel.
        // it should `move` the `tx` channel and we should store the `rx` channel
        // `std::sync::oneshot` is nightly-only and experiemental, so just mimic with a normal mpsc
        let (tx, rx) = mpsc::channel::<Result<Catalog, CatalogBuildError>>(1);
        tokio::task::spawn(async move {
            tracing::info!("Creating GeneratingView");
            let res = builder.build().await;
            match &res {
                Ok(_)  => tracing::info!("[GenerationThread] Successfully generated catalog"),
                Err(e) => tracing::error!("[GenerationThread] Failed to generate catalog: {e:?}"),
            }
            tx.send(res).await;
            tracing::info!("[GenerationThread] Sending result in channel");
        });

        Self {
            build_thread_rx: rx,
            completed: None,
            block: None,
            display: RefCell::new(display)
        }
    }
    pub async fn poll(&mut self) -> Result<(), GeneratingWidgetError> {
        let mut borrow_mut = self.display.borrow_mut();
        select! {
            res = self.build_thread_rx.recv(), if self.completed.is_none() => {
                tracing::info!("Worker Thread complete");
                match res {
                    Some(res) => {
                        tracing::info!("Channel was not closed. Extracting result from channel.");
                        self.completed = Some(res.map_err(|e| GeneratingWidgetError::CatalogGenError(e)));
                    }
                    None => {
                        // tracing::error!("Failed to join thread while generating media catalog! Error: {:?}", e);
                        // if e.is_cancelled() {
                        //     self.completed = Some(Err(GeneratingWidgetError::ManuallyAborted(e)))
                        // } else {
                        //     self.completed = Some(Err(GeneratingWidgetError::PanicOnBuilderThread(e)))
                        // }
                        tracing::info!("Channel was closed before catalog could be extracted. (terminated by user?)");
                        self.completed = Some(Err(GeneratingWidgetError::ChannelClosed))
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
    pub fn block(&mut self, block: Block<'static>) {
        let block = block
            .title_top(Line::from(" Generating Media Catalog ").left_aligned())
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy)
            // .fg(Color::Rgb(153, 121, 61))
        ;
        self.block = Some(block);
    }
    /// doesn't necessarily mean that the builder completed with success
    pub fn is_complete(&self) -> bool { self.completed.is_some() }
    /// Take the completed result from the worker thread.
    pub fn take(&mut self) -> Option<Result<Catalog,GeneratingWidgetError>> { self.completed.take() }
    pub fn received_error(&self) -> bool     { self.display.borrow().received_error() }
}

struct MessageDisplay {
    pub inner: Option<Buffer>,
    pub last: (Option<String>, Option<String>),
    pub subscriber: mpsc::Receiver<Result<String, CatalogBuildError>>,
    has_received_error: bool
}
impl MessageDisplay {
    pub fn new(subscriber: mpsc::Receiver<Result<String, CatalogBuildError>>) -> Self {
        Self {
            inner: None,
            last: (None, None),
            subscriber,
            has_received_error: false,
        }
    }
    pub fn generate(&mut self, rect: Rect)   { self.inner = Some(Buffer::empty(rect)) }
    pub fn is_unset(&self) -> bool           { self.inner.is_none() }
    pub fn set(&mut self, inner: Buffer)     { self.inner = Some(inner) }
    pub fn take(&mut self) -> Option<Buffer> { self.inner.take() }
    pub async fn poll(&mut self) -> Result<(), GeneratingWidgetError> {
        select! {
            res = self.subscriber.recv() => {
                if res.is_none() {
                    return Err(GeneratingWidgetError::PollAfterCompletion);
                }
                let res = res.unwrap();
                match res {
                    Ok(message) => {
                        tracing::info!("[MessageDisplay] Message Received from : {}", message);
                        self.last=(self.last.1.clone(), Some(message.clone())); 
                    },
                    Err(e) => {
                        tracing::warn!("[MessageDisplay] Received error from sender: {:?}", e);
                        self.has_received_error = true;
                        self.last=(self.last.1.clone(), Some(format!("Received error from sender: {:?}", e))); 
                    },
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                tracing::info!("Timed out waiting for GeneratingView subscriber!!");
            }
        }
        Ok(())
    }
    pub fn received_error(&self) -> bool     { self.has_received_error }
}

impl WidgetRef for GeneratingWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized {
        let mut borrow_mut = self.display.borrow_mut();
        self.block.as_ref().render(area, buf);
        let inner = self.block.inner_if_some(area);
        borrow_mut.render(inner, buf);
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

        let mut new_inner = {
            // we take the lines `1..N`, dropping the 0th line, and pad the end of the buffer so it's the right size.
            // ratatui doesn't expose any helpful way to intersect two buffers. `Buffer::merge` just unions two 
            // of them together and exposes no other methods of boolean-ing them together, so there's no way 
            // to be clever about doing this
            let no_border = buffer.area;
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

        //* We need to apply the style to the internal buffer and 
        //* not to `buf` because we use `Buffer::merge`
        // TODO: have some way to inherit the style from the main UI.
        // Maybe just use a global `Styles` like the comment in `media_list.rs` suggests
        new_inner.set_style(buffer.area, Color::Rgb(153, 121, 61));

        Widget::render(Clear, buffer.area, buf);
        let dummy = Paragraph::new("")
            .bold()
            // .block(popup_block.clone())
        ;
        Widget::render(dummy, buffer.area, buf);

        if self.last.1.is_none() { return; }
        let paragraph = Paragraph::new(format!("{}", self.last.1.as_ref().unwrap()))
            .bold()
        ;
        let size = new_inner.area.clone().as_size();
        let pos = new_inner.area.clone().as_position();
        let write_area = Rect::new(pos.x, pos.y+size.height-1, size.width, 1); // last line of buffer
        Widget::render(&paragraph, write_area, &mut new_inner);            // render paragraph to last line of buffer
        // tracing::info!("new_inner:\n{:?}", new_inner);
        buf.merge(&new_inner);
        // Widget::render(&paragraph, write_area, buf);

        // need to re-create a buffer of the original size so that we 
        // don't recursively make the buffer smaller and smaller
        new_inner.resize(buffer.area().clone());
        self.inner = Some(new_inner);
    }
}
