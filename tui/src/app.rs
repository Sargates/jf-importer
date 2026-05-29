use std::io;
use crossterm::{
    event::{self, KeyEventKind, KeyCode, Event, KeyEvent},
};
use ratatui::style::Stylize;
use ratatui::{
    *,
    layout::*,
    text::Line,
    widgets::*,
    style::Color
};

use jf_import_library::{catalog_tree::TreeGenError, config::{CONFIG, ConfigLoadError}};

use crate::tree_view::{self, TreeView};
use crate::logging::*;
use crate::trace_dbg;

pub trait Renderable {
    fn render(&mut self, frame: &mut ratatui::Frame);
    fn handle_input(&mut self, key: crossterm::event::KeyCode);
}

#[derive(Default, Debug)]
enum ErrorCatch {
    #[default]
    NoError,
    FailedToGenerateTree(TreeGenError)
}
impl PartialEq for ErrorCatch {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}
#[derive(Default)]
enum AppState {
    #[default]
    Postinit,
    TreeView(tree_view::TreeView)
}
#[derive(Default)]
pub struct App {
    state: AppState,
    error: ErrorCatch,
    exit: bool,
}
impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }
    fn draw(&mut self, frame: &mut Frame) {
        match &mut self.state {
            AppState::Postinit => self.post_init(frame),
            AppState::TreeView(view) => view.render(frame)
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
            format!("<{}>",KeyCode::Char('p')).blue().bold(),
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
                format!("We failed to load your configuration and had to revert to the default.\nError: {:?}\n{:#?}", CONFIG.load_error, CONFIG.clone()))
                .bold()
                .fg(Color::Red)
                .block(popup_block);
            frame.render_widget(paragraph, float);
        }
    }
    fn render_tree_view(&mut self, tree: &mut TreeView, frame: &mut Frame) {
        tree.render(frame);
    }
    fn handle_events(&mut self) -> io::Result<()> {
        // switch this to crossterm::event::poll
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => {
                if CONFIG.load_error != ConfigLoadError::Success {
                    if let Err(e) = CONFIG.clone().write_to_file() {
                        trace_dbg!("Failed to write config!");
                    }
                }
                self.exit()
            }
            KeyCode::Char('p') => {
                let tree = match TreeView::new() {
                    Ok(tree) => tree,
                    Err(err) => {
                        self.error = ErrorCatch::FailedToGenerateTree(err);
                        return;
                    }
                };
                self.state = AppState::TreeView(tree);
            }
            _ => {
                match &mut self.state {
                    AppState::Postinit => {},
                    AppState::TreeView(view) => view.handle_input(key_event.code)
                }
            }
        }
    }
    fn exit(&mut self) { self.exit = true; }
}
