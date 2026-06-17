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

use std::cell::RefCell;

use jf_import_library::api::{client::QueryStatus, error::QueryError};
use jf_import_library::media::{
    Catalog, CatalogBuilder, CatalogBuildError,
};
use jf_import_library::media;
use jf_import_library::config::*;

use super::{BrailleLoadingIcon, Renderable};

#[derive(Debug)]
pub enum GeneratingViewError {
    GeneratingThreadPanic(tokio::task::JoinError),
    CatalogGenError(CatalogBuildError),
    Timeout,
    RecvError(watch::error::RecvError)
}

pub struct GeneratingView {
    thread_handle: JoinHandle<Result<Catalog, CatalogBuildError>>,
    subscriber: watch::Receiver<String>,
    completed: Option<Catalog>,

    last: (String, String),
    buffer: RefCell<ModalBuffer>
}
impl GeneratingView {
    pub fn new() -> Self {
        let builder = CatalogBuilder::new(CONFIG.clone());
        // this subscriber needs to be properly handled after we call `builder.build`
        let subscriber = builder.subscribe();
        let thread_handle = tokio::task::spawn_blocking(move || {
            tracing::info!("Creating GeneratingView");
            let res = builder.build();
            tracing::info!("Finished GeneratingView");
            res
        });
        Self {
            thread_handle,
            subscriber,
            last: (format!("Unset Previous"), format!("Unset")),
            completed: None,
            buffer: Default::default()
        }
    }
    pub async fn poll(&mut self) -> Result<(), GeneratingViewError> {
        select! {
            res = &mut self.thread_handle => {
                match res {
                    Ok(owned) => {
                        tracing::info!("Finished!!!");
                        self.completed = Some(owned.map_err(|e| GeneratingViewError::CatalogGenError(e))?);
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
                        self.last=(self.last.1.clone(), message); 
                    }
                    Err(e) => {
                        tracing::info!("[] Error when receiving from channel: {:?}", e);
                        Err(e).map_err(|e| GeneratingViewError::RecvError(e))?;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn is_complete(&self) -> bool { self.completed.is_some() }
    pub fn take(&mut self) -> Option<Catalog> { self.completed.take() }
}

// making this a doccomment so code has syntax highlighting
/// TODO: This should be its own widget
/// It should contain more than just a single Option<Buffer> because that makes this whole struct is redundant. It should be its own Widget and it should support getting a Buffer that represents the contents within the Border
/// ```rust
/// // self.buffer should become `Option<ModalBuffer>`
/// struct ModalBuffer {
///     inner_buf: Buffer,
///     bordered: Block, // block with a border
///     inner: Box<impl Widget> // or whatever the syntax is
/// }
/// impl ModalBuffer {
///         Self { inner: Buffer::new(area) }
///     }
/// }
/// ```
#[derive(Default)]
struct ModalBuffer {
    inner: Option<Buffer>
}
impl ModalBuffer {
    pub fn new() -> Self 
        { Self{ inner: None } }
    pub fn generate(&mut self, rect: Rect) 
        { self.inner = Some(Buffer::empty(rect)) }
    pub fn is_unset(&self) -> bool 
        { self.inner.is_none() }
    pub fn set(&mut self, inner: Buffer) 
        { self.inner = Some(inner) }
    pub fn take(&mut self) -> Option<Buffer> 
        { self.inner.take() }
}
impl Renderable for &mut GeneratingView {
    fn render(self, frame: &mut ratatui::Frame)
    where
        Self: Sized {
        let mut borrow_mut = self.buffer.borrow_mut();
        let mut buffer = match borrow_mut.take() {
            Some(buffer) => buffer,
            None => {
                let [_, rect] = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .areas(frame.area());
                let [_, rect] = Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .areas(rect);
                borrow_mut.generate(rect);
                borrow_mut.take().unwrap()
            }
        };

        // paths are unique, so messages from the MediaCatalogBuilder are too. this is 
        // just to prevent messy output that a user may notice due to slower crawling
        if self.last.0 == self.last.1 { return; }
        
        let popup_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" Generating Media Catalog ")
            .merge_borders(symbols::merge::MergeStrategy::Fuzzy)
        ;
        let mut new_inner = {
            // we take the lines `1..N`, dropping the 0th line, and pad the end of the buffer so it's the right size.
            // ratatui doesn't expose any helpful way to intersect two buffers. `Buffer::merge` just unions two 
            // of them together and exposes no other methods of boolean-ing them together, so there's no way 
            // to be clever about doing this
            let no_border = popup_block.inner(buffer.area);
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

        Widget::render(Clear, buffer.area, frame.buffer_mut());
        let dummy = Paragraph::new("")
            .bold()
            .fg(Color::Green)
            .block(popup_block.clone());
        Widget::render(dummy, buffer.area, frame.buffer_mut());

        let paragraph = Paragraph::new(format!("{}", self.last.1))
            .bold()
            .fg(Color::Green);
        let size = new_inner.area.clone().as_size();
        let pos = new_inner.area.clone().as_position();
        let write_area = Rect::new(pos.x, pos.y+size.height-1, size.width, 1); // last line of buffer
        Widget::render(paragraph, write_area, &mut new_inner);            // render paragraph to last line of buffer
        // tracing::info!("new_inner:\n{:?}", new_inner);
        frame.buffer_mut().merge(&new_inner);

        // need to re-create a buffer of the original size so that we 
        // don't recursively make the buffer smaller and smaller
        new_inner.resize(buffer.area().clone());
        borrow_mut.set(new_inner)
    }

    fn handle_input(self, key: crossterm::event::KeyCode) {}
}
