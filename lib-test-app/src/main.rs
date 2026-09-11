// shut up rust analyzer!
#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused)]

mod config;
use config::*;

use std::sync::Arc;

use jfi::catalog::{*, tag_extract};
use jfi::api::client::{ApiClient,TMDBClient};
use jfi::config::*;
use jfi::catalog::*;

use tokio::time::Duration;

use tokio::sync::mpsc;
use tokio::select;

use ignore::*;
use regex::Regex;

#[derive(Debug)]
enum BuildResult {
    Failed,
    SomeErrors,
    Worked,
}

struct TestBuilder {
    tx: mpsc::Sender<String>,
    rx: Option<mpsc::Receiver<String>>,
}
impl TestBuilder {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<String>(1000);
        Self {
            tx,
            rx: Some(rx)
        }
    }
    pub async fn build(self) -> Result<(), BuildResult> {
        for i in 1..100 {
            self.tx.send(format!("Gaming #{}!", i)).await.unwrap();
        }
        println!("Completed");
        Ok(())

    }
    pub fn subscribe(&mut self) -> mpsc::Receiver<String> {
        self.rx.take().unwrap()
    }
}

#[tokio::main]
async fn main() {
    let config = match JfiConfig::load_config() {
        Ok(c) => {
            Some(c)
        }
        Err(e) => {
            println!("Failed to load config from file with reason: {:?}", e);
            JfiConfig::default_config()
        }
    };

    if let None = config {
        eprintln!("Failed to locate home directory");
        return;
    }
    let config = config.unwrap();

    let config: Config = match config.try_into() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create valid config: {:?}", e);
            return;
        }
    };
    let config = Arc::new(config);
    println!("{:#?}", config);

    let (tx, mut rx) = mpsc::channel::<Result<String, CatalogBuildError>>(1000);
    let mut builder = CatalogBuilder::new(config.clone(), Some(tx));
    // let mut builder = TestBuilder::new();
    
    // let mut rx = builder.subscribe();

    let mut thread_handle = tokio::task::spawn(async move {
        println!("Creating GeneratingView");
        let res = builder.build().await;
        match &res {
            Ok(_)  => println!("[GenerationThread] Successfully generated catalog"),
            Err(e) => println!("[GenerationThread] Failed to generate catalog: {e:?}"),
        }
        res
    });

    let mut catalog: Option<Catalog> = None;

    loop {
        select! {
            thing = &mut thread_handle => {
                println!("Finished");
                catalog = Some(thing.unwrap().unwrap());
                break;
            }
            res = rx.recv(), if !rx.is_closed() && !rx.is_empty() => {
                if let None = res {
                    println!("Failed");
                    break;
                }
                let res = res.unwrap();
                // println!("Received: {:?}", res);
            }
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
    }

    let thing = catalog.unwrap();

    for media in thing.iter_all() {
        match media {
            MediaItem::Movie(movie) => println!("{:?}", movie),
            MediaItem::Show(show)   => println!("{:?}", show),
            MediaItem::Episode(ep)  => println!("{:?}", ep),
        }
    }

}
