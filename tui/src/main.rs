// shut up rust analyzer!
#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused)]

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

use std::fs;

use jf_import_library::config::*;

mod app;
mod tree_view;
mod logging;

use app::App;
use logging::*;

fn post_init() {
    // Post-init checks
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install()?;
    initialize_logging()?;
    println!("DATA_FOLDER: {:?}", DATA_FOLDER.clone().unwrap());
    println!("LOG_ENV: {}", LOG_ENV.clone());
    println!("LOG_FILE: {}", LOG_FILE.clone());
    println!("data_dir: {:?}", get_data_dir());

    ratatui::run(|terminal| App::default().run(terminal))?;
    
    // let file = "Test.toml";
    // let conf = CONFIG.clone();
    // let string: String = toml::to_string(&conf).unwrap();
    //
    // if let Err(err) = fs::write(file, string) {
    //     println!("Failed to write file! Error: {}", err);
    //     return Ok(());
    // }
    //
    // let content = fs::read_to_string(file).unwrap();
    // let from_file: Config = toml::from_str(&content).unwrap();
    //
    // assert!(conf == from_file);
    // println!("{:#?}", conf);

    Ok(())
}

// fn render(frame: &mut Frame) {
//     let title = Line::from(" Counter App Tutorial ".bold());
//     let instructions = Line::from(vec![
//         " Decrement ".into(),
//         format!("<{}>",KeyCode::Left).blue().bold(),
//         " Increment ".into(),
//         format!("<{}>",KeyCode::Right).blue().bold(),
//         " Quit ".into(),
//         format!("<{}>",KeyCode::Char('q')).to_ascii_uppercase().blue().bold(),
//     ]);
//
//     let outer_layout = Layout::default()
//         .direction(layout::Direction::Vertical)
//         .margin(1)
//         .constraints(vec![
//             Constraint::Percentage(50),
//             Constraint::Percentage(50)])
//         .split(frame.area());
//
//     frame.render_widget(
//         Paragraph::new("outer 0")
//         .block(Block::new().bold().fg(Color::Red).borders(Borders::ALL).title_bottom(instructions.clone().centered())),
//         outer_layout[0]
//     );
//     frame.render_widget(
//         Paragraph::new("outer 1")
//         .block(Block::new().bold().fg(Color::Yellow).borders(Borders::ALL).title_bottom(instructions.clone().centered()))
//         ,
//         outer_layout[1]
//     );
// }

