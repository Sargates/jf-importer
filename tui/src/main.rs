// shut up rust analyzer!
#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused)]

use std::fs;
use std::sync::Arc;

use futures::{
    stream::{FuturesUnordered, Stream, StreamExt},
    executor::block_on
};
use tokio::{self, main, select};
use ratatui::{crossterm, Terminal, prelude::CrosstermBackend};

mod app;
mod widgets;
mod utils;
mod logging;

use app::App;
use logging::*;

use jf_import_library::api::*;
use jf_import_library::media_item::{Movie, Show, Episode};
use jf_import_library::config::*;

//* TESTING IMPORTS
use jf_import_library::media_catalog::MediaCatalog;

fn post_init() {
    // Post-init checks
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install()?;
    initialize_logging()?;
    // println!("DATA_FOLDER: {:?}", DATA_FOLDER.clone().unwrap());
    println!("LOG_ENV: {}", LOG_ENV.clone());
    println!("LOG_FILE: {}", LOG_FILE.clone());
    println!("data_dir: {:?}", get_data_dir());
    // ''

    println!("CARGO_CRATE_NAME: {}", env!("CARGO_CRATE_NAME"));
    println!("CONFIG.clone(): {:#?}", CONFIG.clone());
    println!("SECRETS.clone(): {:#?}", SECRETS.clone());
    tracing::info!("GAMING!");
    
    // `ratatui::run` expects a synchronous closure, this is just ripped from `ratatui::run` and
    // changed to move `terminal` since it isn't used elsewhere
    let mut terminal = ratatui::init();
    let f = || async move {
        let result = App::default().run(&mut terminal).await;
    };
    f().await;
    ratatui::restore();
    
    // let mut tree = match MediaCatalog::new(CONFIG.clone()).generate_catalog_tree() {
    //     Ok(t) => t,
    //     Err(err) => { panic!("Failed to create catalog tree! Err: {:?}", err); },
    // };
    //
    // let client: Arc<dyn ApiClient + Send + Sync> = Arc::new(TMDBClient::new());
    //
    // let mut api_tasks: FuturesUnordered<_> = FuturesUnordered::new();
    //
    // for (idx, movie) in tree.movies.iter().enumerate() {
    //     let movie = movie.clone();
    //     let client = client.clone();
    //     let future = tokio::task::spawn(async move {
    //         let mut lock = movie.query.lock().await;
    //         *lock = client.search_movie(movie.clone()).await;
    //         let formatted_name = match &*lock {
    //             QueryStatus::Success(query) => {
    //                 query.title.clone()
    //             }
    //             QueryStatus::Failed(err) => {
    //                 println!("Failed to search for Movie! Error: {:?}", err);
    //                 String::from("Unknown")
    //             }
    //             QueryStatus::NotStarted => {
    //                 String::from(format!("[NotStarted] {}", movie.src.to_string_lossy()))
    //             }
    //             QueryStatus::InProgress => {
    //                 String::from(format!("[InProgress] {}", movie.src.to_string_lossy()))
    //             }
    //         };
    //         format!("Movie #{idx}: {formatted_name}")
    //     });
    //     api_tasks.push(future);
    // }
    //
    // // Iterate over Shows in the catalog
    // for (idx, show) in tree.shows.iter().enumerate() {
    //     let show = show.clone();
    //     let client = client.clone();
    //     let future = tokio::task::spawn(async move {
    //         let mut lock = show.query.lock().await;
    //         *lock = client.search_show(show.clone()).await;
    //         let formatted_name = match &*lock {
    //             QueryStatus::Success(query) => {
    //                 query.title.clone()
    //             }
    //             QueryStatus::Failed(err) => {
    //                 println!("Failed to search for Movie! Error: {:?}", err);
    //                 String::from("Unknown")
    //             }
    //             QueryStatus::NotStarted => {
    //                 String::from(format!("[NotStarted] {}", show.src.to_string_lossy()))
    //             }
    //             QueryStatus::InProgress => {
    //                 String::from(format!("[InProgress] {}", show.src.to_string_lossy()))
    //             }
    //         };
    //         format!("Show #{idx}: {formatted_name}")
    //     });
    //     api_tasks.push(future);
    // }
    //
    // //* Lock testing
    // // { 
    // //     let show_ref = tree.shows.get(5).unwrap().clone();
    // //     let show_guard = block_on(show_ref.lock());
    // //     let ep_ref = show_guard.episodes.get(18).unwrap().clone();
    // //     let ep_guard = ep_ref.try_lock();
    // //     println!("State: {:?}", ep_guard);
    // // }
    //
    // loop {
    //     select! {
    //         Some(result) = api_tasks.next(), if !api_tasks.is_empty() => {
    //             match result {
    //                 Ok(title)       => println!("{}", title),
    //                 Err(join_error) => println!("Failed to join future and main thread! Error: {:?}", join_error),
    //             }
    //         }
    //         timed_out = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
    //             if api_tasks.is_empty() {
    //                 println!("> Breaking...");
    //                 break;
    //             }
    //             println!("Timeout!");
    //         }
    //     }
    //     // println!("Rendering!");
    // }
    //
    // //* Lock testing
    // // { 
    // //     let show_ref = tree.shows.get(5).unwrap().clone();
    // //     let show_guard = block_on(show_ref.lock());
    // //     let ep_ref = show_guard.episodes.get(18).unwrap().clone();
    // //     let ep_guard = ep_ref.try_lock();
    // //     println!("State: {:?}", ep_guard);
    // // }
    //
    // utils::recursive_print(&tree.tree, String::new(), false);
    
    Ok(())
}

