// shut up rust analyzer!
#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused)]

use std::fs;
use std::sync::Arc;

use tokio::{self, main};

mod config;
mod app;
mod widgets;
mod utils;
mod logging;
mod ffprobe;

use app::App;
use logging::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install()?;
    initialize_logging()?;
    // println!("DATA_FOLDER: {:?}", DATA_FOLDER.clone().unwrap());

    println!("LOG_ENV: {}", LOG_ENV.clone());
    println!("LOG_FILE: {}", LOG_FILE.clone());
    println!("data_dir: {:?}", get_data_dir());

    println!("CARGO_CRATE_NAME: {}", env!("CARGO_CRATE_NAME"));
    // println!("CONFIG.clone(): {:#?}", CONFIG.clone());
    // println!("SECRETS.clone(): {:#?}", SECRETS.clone());

    App::new().run().await;

    Ok(())
}

