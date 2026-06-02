use std::{io, time::Duration};
use std::rc::Rc;

use crossterm::{
    event::{self, KeyEventKind, KeyCode, Event, KeyEvent},
};
use futures::FutureExt;
use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Stylize}
};

use jf_import_library::{
    media_catalog::{self, TreeGenError, MediaCatalog},
    config::{CONFIG, ConfigLoadError},
};
use tokio::{self, select, sync::watch};
use futures::executor::block_on;

use crate::widgets::{self, TreeView, GeneratingView, error::*};
use crate::logging::*;
// use crate::trace_dbg;

pub trait Renderable {
    fn render(&mut self, frame: &mut ratatui::Frame);
    fn handle_input(&mut self, key: crossterm::event::KeyCode);
}

#[derive(Default, Debug)]
enum ErrorCatch {
    #[default]
    NoError,
    FailedToGenerateTree(TreeGenError),
    AppUpdateError(AppUpdateError)
}
impl PartialEq for ErrorCatch {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

#[derive(Debug)]
enum AppUpdateError {
    ChannelRecvError(watch::error::RecvError),
    GeneratingViewError(GeneratingViewError),
    TreeViewError(TreeViewError),
}

#[derive(Default)]
enum AppState {
    #[default]
    Postinit,
    GeneratingTree(GeneratingView),
    MillerColumnView(TreeView)
}
#[derive(Default)]
pub struct App {
    tree: Option<Rc<MediaCatalog>>,
    state: AppState,
    error: ErrorCatch,
    exit: bool,
}
impl App {
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        tracing::info!("Starting!!");
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
            select! {
                res = self.update() => {
                    match res {
                        Ok(_) => { /* tracing::info!("No error during update!"); */ },
                        Err(e) => { tracing::error!("An error occured: Error: {:?}", e); }
                    }
                }
                //* I think putting a timeout in this outer `select!` can cause a race condition by 
                //* the timeout trashing whatever work is being done in `update`
                // _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                //     tracing::debug!("Timed out waiting for update");
                //     // self.exit = true;
                // }
            }
            // tracing::info!("Async block finished");
        }
        Ok(())
    }
    async fn update(&mut self) -> Result<(), AppUpdateError> {
        match &mut self.state {
            AppState::GeneratingTree(view) => {
                if view.is_complete() {
                    tracing::info!("View is completed!");
                    let ptr = Rc::new(view.take().unwrap());
                    let tree_view = TreeView::new(ptr.clone()).map_err(|e| AppUpdateError::TreeViewError(e))?;
                    self.tree = Some(ptr);
                    self.state = AppState::MillerColumnView(tree_view);
                    return Ok(())
                }
                
                // tracing::info!("Awaiting update");
                match view.poll().await {
                    Ok(_) => {},
                    Err(err) => { Err(AppUpdateError::GeneratingViewError(err))?; }
                }
            }
            _ => {}
        }
        Ok(futures::future::ready(()).await)
    }
    fn draw(&mut self, frame: &mut Frame) {
        match &mut self.state {
            AppState::Postinit => self.post_init(frame),
            AppState::GeneratingTree(view) => view.render(frame),
            AppState::MillerColumnView(view) => view.render(frame),
        }
        if self.error != ErrorCatch::NoError {
            let popup_block = Block::bordered().title("An Error Occured!");
            let horizontal = Layout::default()
                .direction(layout::Direction::Horizontal)
                .constraints(vec![Constraint::Fill(3), Constraint::Fill(1)])
                .split(frame.area());
            let layout = Layout::default()
                .direction(layout::Direction::Vertical)
                .constraints(vec![Constraint::Fill(3), Constraint::Fill(1)])
                .split(horizontal[1]);

            frame.render_widget(Clear, layout[1]);
            let paragraph = Paragraph::new(format!("Error: {:?}",self.error)).block(popup_block);
            frame.render_widget(paragraph, layout[1]);
        }
    }
    fn post_init(&self, frame: &mut Frame) {
        let area = frame.area();
        let title = Line::from(" Counter App Tutorial ".bold());
        let instructions = Line::from(vec![
            " Decrement ".into(),
            format!("<{}>",KeyCode::Left).blue().bold(),
            " Increment ".into(),
            format!("<{}>",KeyCode::Right).blue().bold(),
            " Quit ".into(),
            format!("<{}>",KeyCode::Char('q')).blue().bold(),
            " Load Catalog ".into(),
            format!("<{}> ",KeyCode::Char('p')).blue().bold(),
        ]);

        let layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Percentage(50),
                Constraint::Percentage(50)
            ]);
        let [top, bottom] = area.layout(&layout);
        frame.render_widget(
            Paragraph::new("outer 0")
                .block(Block::new().bold().fg(Color::Red).borders(Borders::ALL).title_bottom(instructions.clone().centered())),
            top
        );
        frame.render_widget(
            Paragraph::new("outer 1")
                .block(Block::new().bold().fg(Color::Yellow).borders(Borders::ALL).title_bottom(instructions.clone().centered())),
            bottom
        );
        if CONFIG.load_error != ConfigLoadError::Success {
            let popup_block = Block::bordered().title("Failed to load configuration!");
            let float = area.centered(Constraint::Percentage(60), Constraint::Percentage(20));

            frame.render_widget(Clear, float);
            let paragraph = Paragraph::new(
                format!("I failed to load your configuration and had to revert to the default.\nError: {:?}\n{:#?}", CONFIG.load_error, CONFIG.clone()))
                .bold()
                .fg(Color::Red)
                .block(popup_block);
            frame.render_widget(paragraph, float);
        }
    }
    fn handle_events(&mut self) -> io::Result<()> {
        // switch this to crossterm::event::poll
        if event::poll(Duration::from_millis(0))? {
            match event::read()? {
                // it's important to check that the event is a key press event as
                // crossterm also emits key release and repeat events on Windows.
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            }
        }

        Ok(())
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => {
                if CONFIG.load_error != ConfigLoadError::Success {
                    if let Err(e) = CONFIG.clone().write_to_file() {
                        tracing::error!("Failed to write config!");
                    }
                }
                self.exit()
            }
            KeyCode::Char('p') => {
                let res = GeneratingView::new();
                self.state = AppState::GeneratingTree(res)
            }
            _ => {
                match &mut self.state {
                    AppState::Postinit => {},
                    AppState::GeneratingTree(view) => {}, // no user input
                    AppState::MillerColumnView(view) => view.handle_input(key_event.code)
                }
            }
        }
    }
    fn exit(&mut self) { self.exit = true; }
}
