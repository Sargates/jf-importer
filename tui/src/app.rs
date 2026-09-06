use std::error::Error;
use std::{io, time::Duration};
use std::rc::Rc;
use std::sync::Arc;

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


use jfi::{
    media,
    api::client::TMDBClient,
    catalog::*,
    config::*,
};

use tokio::{self, select, sync::watch};
use futures::executor::block_on;

use crate::widgets::{self, *, error::*};
use crate::logging::*;
use crate::config::*;

use self::AppState::RevertedToDefaultConfig;
use self::KilledBy::NaturalCauses;

#[derive(Default, Debug)]
enum KilledBy {
    #[default]
    NaturalCauses,
    FailedToRevertToDefaultConfig, // unrecoverable, for now
}

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
    CatalogView(CatalogView),
    RevertedToDefaultConfig,
    InvalidConfigSupplied,
}

pub struct App {
    config: Option<Arc<jfi::Config>>,
    tree: Option<Rc<Catalog>>,
    state: AppState,
    error: ErrorCatch,
    exit: Option<KilledBy>,
}


impl App {
    pub fn new() -> App {
        let jfi_config = JfiConfig::load_config();

        let (state, config) = if let Ok(cfg) = jfi_config {
            match <JfiConfig as TryInto<jfi::Config>>::try_into(cfg) {
                Ok(c) => {
                    let cfg = Arc::new(c);
                    (AppState::CatalogView(CatalogView::new(cfg.clone())), Some(cfg))
                }
                Err(e) => {
                    tracing::info!("Config loaded from file is not a valid library configuration. Error: {:?}", e);
                    (AppState::InvalidConfigSupplied, None)
                }
            }
        }
        else { (AppState::RevertedToDefaultConfig, None) };

        let exit = if let None = config {
               Some(KilledBy::FailedToRevertToDefaultConfig) }
        else { None };

        App {
            config,
            tree: None,
            state,
            error: ErrorCatch::NoError,
            exit,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        tracing::info!("Starting!!");
        
        // `ratatui::run` expects a synchronous closure, this is just ripped from `ratatui::run` and
        // changed to move `terminal` since it isn't used elsewhere
        let mut terminal = ratatui::init();

        while let None = self.exit {
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

        ratatui::restore();

        let death = self.exit.take().unwrap();
        if let KilledBy::NaturalCauses = death {}
        else {
            println!("Exited with Error: {:?}", death);
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

        // cycle the static braille widget
        match BRAILLE.try_lock() {
            Ok(mut lock) => lock.tick_next(),
            Err(err) => tracing::error!("Failed to acquire lock for ticking braille widget: Err: {}", err),
        };
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        match &mut self.state {
            AppState::CatalogView(view)       => { view.render(frame) },
            AppState::RevertedToDefaultConfig => { ConfigView::render(frame) },
            AppState::InvalidConfigSupplied => {},
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
        tracing::info!("Pressed: {:?}", key_event.code);
        match key_event.code {
            KeyCode::Char('q') => {
                // if CONFIG.load_error != SecretsLoadError::Success {
                //     if let Err(e) = CONFIG.clone().write_to_file() {
                //         tracing::error!("Failed to write config!");
                //     }
                // }
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
                    AppState::CatalogView(view) => view.handle_input(key_event),
                    AppState::RevertedToDefaultConfig => {},
                    AppState::InvalidConfigSupplied => {},
                }
            }
        }
    }
    fn exit(&mut self) { self.exit = Some(NaturalCauses); }
}
