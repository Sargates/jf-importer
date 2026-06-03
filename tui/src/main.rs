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

fn post_init() {
    // Post-init checks
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install()?;
    initialize_logging()?;
    // println!("DATA_FOLDER: {:?}", DATA_FOLDER.clone().unwrap());
    // println!("LOG_ENV: {}", LOG_ENV.clone());
    // println!("LOG_FILE: {}", LOG_FILE.clone());
    // println!("data_dir: {:?}", get_data_dir());
    // ''

    println!("CARGO_CRATE_NAME: {}", env!("CARGO_CRATE_NAME"));
    println!("CONFIG.clone(): {:#?}", CONFIG.clone());
    println!("SECRETS.clone(): {:#?}", SECRETS.clone());
    
    // `ratatui::run` expects a synchronous closure, this is just ripped from `ratatui::run` and
    // changed to move `terminal` since it isn't used elsewhere
    let mut terminal = ratatui::init();
    let f = || async move {
        let result = App::default().run(&mut terminal).await;
    };
    f().await;
    ratatui::restore();
    
    // ratatui::run(|terminal| App::default().run(terminal))?;
    
    // let mut tree = match dir_search::generate_catalog_tree() {
    //     Ok(t) => t,
    //     Err(err) => { panic!("Failed to create catalog tree! Err: {:?}", err); },
    // };
    // // println!("{}", std::env::var("VIDEO_FILE_EXTENTIONS").unwrap());
    //
    // let client: Arc<dyn ApiClient + Send + Sync> = Arc::new(TMDBClient::new());
    //
    // let mut api_tasks: FuturesUnordered<_> = FuturesUnordered::new();
    //
    // for (idx, movie) in tree.movies.iter().enumerate() {
    //     let movie = movie.clone();
    //     let client = client.clone();
    //     let future = tokio::task::spawn(async move {
    //         let formatted_name = match client.search_movie(movie.clone()).await {
    //             Ok(_) => { movie.lock().await.query.as_ref().unwrap().title.clone() },
    //             Err(err) => {
    //                 println!("Failed to search for Movie! Error: {:?}", err);
    //                 String::from("Unknown")
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
    //         let formatted_name = match client.search_show(show.clone()).await {
    //             Ok(item) => show.lock().await.query.as_ref().unwrap().title.clone(),
    //             Err(err) => {
    //                 println!("Failed to search for Movie! Error: {:?}", err);
    //                 show.lock().await.src.to_string_lossy().to_string() 
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

