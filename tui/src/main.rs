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

use jf_import_library::{config::*, dir_search};

mod app;
mod tree_view;
mod logging;

use app::App;
use logging::*;

use jf_import_library::api_query::{Queryable, QueryError};
use jf_import_library::media_item::{Movie, Show, Episode};

use reqwest::blocking::{
    Client,
    RequestBuilder,
    Response,
};

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

    println!("{:#?}", CONFIG.clone());
    println!("{:#?}", SECRETS.clone());

    ratatui::run(|terminal| App::default().run(terminal))?;

    // let mut tree = match dir_search::generate_catalog_tree() {
    //     Ok(t) => t,
    //     Err(err) => { panic!("Failed to create catalog tree! Err: {:?}", err); },
    // };
    //
    // let client = Client::new();
    //
    // // println!("{}", std::env::var("VIDEO_FILE_EXTENTIONS").unwrap());
    //
    // // Iterate over Movies in the catalog
    // for movie in tree.movies {
    //     let res = movie.borrow_mut().query_api(&client);
    //     if let Err(err) = res {
    //         println!("Query Failure: {err:?} for {}", movie.borrow().src.to_string_lossy());
    //         continue;
    //     }
    //     let response = res.unwrap();
    //     let status = movie.borrow_mut().process_response(response);
    //     if let Err(err) = status {
    //         println!("JSON Processing Failure: {err:?} for {}", movie.borrow().src.to_string_lossy());
    //         continue;
    //     }
    // }
    //
    // // Iterate over Shows in the catalog
    // for show in tree.shows {
    //     let res = show.borrow_mut().query_api(&client);
    //     if let Err(err) = res {
    //         println!("Query Failure: {err:?} for {}", show.borrow().src.to_string_lossy());
    //         continue;
    //     }
    //     let response = res.unwrap();
    //     let status = show.borrow_mut().process_response(response);
    //     if let Err(err) = status {
    //         match err {
    //             QueryError::FailedToExtractApiData(json) => println!("JSON Processing Failure: {json}"),
    //             _                                        => println!("Failed to process API response: {err:?}"),
    //         }
    //         continue;
    //     }
    // }
    //
    // tree_view::recursive_print(&tree.tree, String::new());
    
    Ok(())
}

