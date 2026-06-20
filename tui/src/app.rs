use std::{io, time::Duration};
use std::rc::Rc;
use std::sync::Arc;

use crossterm::{
    event::{self, KeyEventKind, KeyCode, Event, KeyEvent},
};

use futures::FutureExt;
use jf_import_library::api::client::TMDBClient;
use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::{Color,Stylize}
};

use jf_import_library::{
    media::{self, CatalogBuildError, Catalog},
    config::{CONFIG, ConfigLoadError},
};

use tokio::{self, select, sync::watch};
use futures::executor::block_on;

use crate::widgets::{self, Renderable, CatalogView, GeneratingWidget, error::*};
use crate::logging::*;

#[derive(Default, Debug)]
enum ErrorCatch {
    #[default]
    NoError,
    FailedToGenerateCatalog(CatalogBuildError),
    AppUpdateError(AppUpdateError)
}

#[derive(Debug)]
enum AppUpdateError {
    ChannelRecvError,
    GeneratingViewError(GeneratingWidgetError),
    CatalogViewError(TreeViewError),
}

enum AppState {
    // #[default]
    // Postinit,
    // GeneratingCatalog(GeneratingView),
    CatalogView(CatalogView)
}
// can't derive for non-unit variants
impl Default for AppState {
    fn default() -> Self {
        Self::CatalogView(
            CatalogView::default()
                .with_client(Arc::new(TMDBClient::new()))
        )
    }
}

#[derive(Default)]
pub struct App {
    tree: Option<Rc<Catalog>>,
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
                //* the timeout trashing whatever work is being done in `update`.
                //* Maybe not because we're not `await`ing across writes; control is never
                //* relinquished while writing so we're never in an undefined state
                // TODO: FITFO by testing
                // _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                //     tracing::debug!("Timed out waiting for update");
                //     // self.exit = true;
                // }
            }
        }
        Ok(())
    }
    async fn update(&mut self) -> Result<(), AppUpdateError> {
        match &mut self.state {
            AppState::CatalogView(view) => {
                match view.update().await {
                    Ok(_) => {}
                    Err(e) => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn draw(&mut self, frame: &mut Frame) {
        match &mut self.state {
            AppState::CatalogView(view) => { view.render(frame) },
        }

        match self.error {
            ErrorCatch::NoError => {}
            _ => {
                let popup_block = Block::bordered().title("An Error Occured!");
                let horizontal = Layout::default()
                    .direction(layout::Direction::Horizontal)
                    .constraints(vec![Constraint::Fill(3), Constraint::Fill(1)])
                    .split(frame.area());
                let layout = Layout::default()
                    .direction(layout::Direction::Vertical)
                    .constraints(vec![Constraint::Fill(3), Constraint::Fill(1)])
                    .split(horizontal[1]);

                Clear.render(layout[1], frame.buffer_mut());
                let paragraph = Paragraph::new(format!("Error: {:?}",self.error)).block(popup_block);
                paragraph.render(layout[1], frame.buffer_mut())
            }
        }
    }
    fn default_background(&self, frame: &mut Frame) {
        let area = frame.area();
        let title = Line::from(" Counter App Tutorial ".bold());
        let instructions = Line::from(vec![
            " ".into(),
            "Decrement ".into(),
            format!("<{}>",KeyCode::Left).blue().bold(),
            " Increment ".into(),
            format!("<{}>",KeyCode::Right).blue().bold(),
            " Quit ".into(),
            format!("<{}>",KeyCode::Char('q')).blue().bold(),
            " Load Catalog ".into(),
            format!("<{}>",KeyCode::Char('p')).blue().bold(),
            " ".into(),
        ]);

        let layout = Layout::vertical([Constraint::Percentage(50); 2]);
        let [top, bottom] = area.layout(&layout);
        frame.render_widget(
            Paragraph::new("outer 0")
                .block(Block::new().bold().fg(Color::Red).borders(Borders::ALL).title_top(title.clone()).title_bottom(instructions.clone().centered())),
            top
        );
        frame.render_widget(
            Paragraph::new("outer 1")
                .block(Block::new().bold().fg(Color::Yellow).borders(Borders::ALL).title_top(title.clone()).title_bottom(instructions.clone().centered())),
            bottom
        );
        if CONFIG.load_error != ConfigLoadError::Success {
            let popup_block = Block::bordered().title("Failed to load configuration!");
            let float = area.centered(Constraint::Percentage(60), Constraint::Percentage(20));

            Clear.render(float, frame.buffer_mut());
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
            _ => {
                match &mut self.state {
                    // AppState::Postinit => {
                    //     match key_event.code {
                    //         KeyCode::Char('p') => {
                    //             let res = GeneratingView::new();
                    //             self.state = AppState::GeneratingCatalog(res)
                    //         }
                    //         _ => {}
                    //     }
                    // }
                    // AppState::GeneratingCatalog(view) => {}, // no user input
                    AppState::CatalogView(view) => view.handle_input(key_event.code)
                }
            }
        }
    }
    fn exit(&mut self) { self.exit = true; }
}
