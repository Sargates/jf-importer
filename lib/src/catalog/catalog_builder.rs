use std::time::Duration;
use std::{ fmt, fs, io };
use std::env;
use std::ffi::OsString;
use std::path::{ Path, PathBuf };
use std::sync::{Arc, Weak};
use std::sync::mpsc::*;

use regex::Regex;
use ignore::*;

use tokio::sync::{Mutex, mpsc};

use crate::api::client::ApiClient;
use crate::catalog::*;
use crate::media::{
    Episode,
    MediaItem,
    Movie,
    Show,
    MediaCreateError
};

use crate::api::{
    calls::ApiManifest,
    client::*
};
use crate::config::{Config,Secrets,SECRETS};

#[derive(Debug, Clone)]
pub enum CatalogBuildError {
    EnvVarNotSet,
    EnvVarNotUnicode(OsString),

    RegexError(regex::Error),
    // StatusReportSendError(mpsc::error::SendError<String>)
}
impl From<regex::Error> for CatalogBuildError {
    fn from(value: regex::Error) -> Self { Self::RegexError(value) }
}
// impl From<mpsc::error::SendError<String>> for CatalogBuildError {
//     fn from(value: mpsc::error::SendError<String>) -> Self { Self::StatusReportSendError(value) }
// }

/// Raw intermediate type for catalog builder
enum RawCatalogItem {
    Show(String),
    Movie(String),
    Episode(String),
}

pub struct CatalogBuilder {
    config: Arc<Config>,
    status_tx: Option<mpsc::Sender<Result<String, CatalogBuildError>>>,

    client: Option<Arc<dyn ApiClient + Sync + Send>>,

    // items: Vec<RawCatalogItem>,
    movies: Vec<Arc<Movie>>,
    shows: Vec<Arc<Show>>,
    episodes: Vec<Arc<Episode>>,
    fails: Vec<CatalogFailure>,
}

pub struct CatalogFailure {
    pub err: MediaCreateError,
    pub buf: PathBuf
}

enum ParseAs {
    Movie,
    Show,
    // Episode,
}

impl CatalogBuilder {
    pub fn new(config: Arc<Config>, status_tx: Option<mpsc::Sender<Result<String, CatalogBuildError>>>) -> Self {
        Self {
            config,
            client: None,
            status_tx,
            movies: Vec::new(),
            shows: Vec::new(),
            episodes: Vec::new(),
            fails: Vec::new(),
        }
    }

    pub fn with_client(mut self, client: Option<Arc<dyn ApiClient+Sync+Send>>) -> CatalogBuilder { self.client = client; self }
    pub fn set_client(&mut self, client: Option<Arc<dyn ApiClient+Sync+Send>>) { self.client = client; }

    // TODO: Change this to use an intermediate struct for each item.
    // Do a full walk while pushing entries to an enum. Then, split each entry
    // type into its own vector while collecting repeats into a single
    // `MediaItem` (would require changing the definition of `MediaItem`) as a
    // structure of arrays.
    pub async fn build(mut self) -> Result<Catalog, CatalogBuildError> {
        let config = self.config.clone();
        let ingest_movies_dir = &config.IngestMovieDir;
        let ingest_shows_dir  = &config.IngestShowDir;

        let dst_movies_dir = &config.JfMovieDir;
        let dst_shows_dir  = &config.JfShowDir;

        let ignore_re = Regex::new(r".*(ignore|temp).*")?;
        let video_re = Regex::new(self.config.VideoFileExtensions.as_str())?;

        // let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
        // let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
        // let is_match = |x: &String, re: &Regex| re.is_match(x);

        // I HAVE NEVER GOTTEN TO USE CURRYING BEFORE!!! LET'S GO
        let currying_match = |re: Regex, invert: bool| 
            move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) };

        let ignore_filter = currying_match(ignore_re.clone(), true);
        let video_filter = currying_match(video_re.clone(), false);

        // Walk directory containing unimported movies
        let mut ingest_movie_walkbuilder = WalkBuilder::new(ingest_movies_dir.clone());
        ingest_movie_walkbuilder
            .min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(ignore_filter.clone())
            .filter_entry(video_filter.clone());
        self.parse_walk(ingest_movie_walkbuilder, ParseAs::Movie).await;

        // Walk directory containing unimported movies
        let mut ingest_show_builder = WalkBuilder::new(ingest_shows_dir.clone());
        ingest_show_builder
            .min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(ignore_filter.clone());
        self.parse_walk(ingest_show_builder, ParseAs::Show).await;

        //* Note that we don't use `ignore_filter` while walking the already-imported media
        //* We don't want to exclude something that might have the word "ignore" in the title

        // Walk the already-imported jellyfin movies
        let mut imported_movie_walkbuilder = WalkBuilder::new(dst_movies_dir.clone());
        imported_movie_walkbuilder
            .min_depth(Some(2))
            .max_depth(Some(2))
            .filter_entry(video_filter.clone());
        self.parse_walk(imported_movie_walkbuilder, ParseAs::Movie).await;

        // Walk the already-imported jellyfin shows
        let mut imported_show_walkbuilder = WalkBuilder::new(dst_shows_dir.clone());
        imported_show_walkbuilder
            .min_depth(Some(1))
            .max_depth(Some(1));
        self.parse_walk(imported_show_walkbuilder, ParseAs::Show).await;

        Ok(Catalog {
            config: self.config,
            movies: self.movies,
            shows: self.shows,
            episodes: self.episodes,
            failures: self.fails,
            api_client: self.client,
            api_manifest: Mutex::new(ApiManifest::default()),
        })
    }

    async fn parse_walk(&mut self, builder: WalkBuilder, parse_type: ParseAs) {
        for result in builder.build() {
            if let Err(err) = result {
                tracing::error!("Bad Dir Entry {}", err);
                continue;
            }
            let dir_entry = result.unwrap();
            if match &parse_type {
                ParseAs::Movie => { ! dir_entry.path().is_file() }
                ParseAs::Show => { ! dir_entry.path().is_dir() }
            } {
                continue;
            }

            self.push_item(dir_entry, &parse_type).await;
        }
    }

    async fn push_item(&mut self, dir_entry: DirEntry, parse: &ParseAs) {
        //* This was only intended to ensure that all messages to the status 
        //* channel would get logged. May or may not be relevant in the future. 
        //* The fact that the worker thread outpaces the render thread *does not* 
        //* affect the catalog from processing all media items, only the reporting 
        //* of doing so.
        // if self.what_i_am_doing.capacity() < 30 {
        //     // let the receiver thread catch up
        //     // on performance implications: this isn't as bad as it looks.
        //     // the worker thread just outpaces the UI thread, and the mpsc 
        //     // still buffers 1000 messages. So this only activates for 
        //     // larger catalogs
        //     std::thread::sleep(Duration::from_millis(500));
        // }

        // The worker thread created to build the catalog actually outpaces 
        // the render thread, but as a result we skip messages from the 
        let label = match parse {
            ParseAs::Movie   => "[Movie]  ",
            ParseAs::Show    => "[Show]   ",
            // ParseAs::Episode => "[Ep]     ",
        };

        if let Some(ref tx) = self.status_tx &&
           let Err(e) = tx.send(Ok(format!("{label} Found: {}", dir_entry.file_name().to_string_lossy().to_string()))).await {
            tracing::info!("[CatalogBuilder::push_item] Failed to send message on channel. (destroyed?)");
            return;
        }

        // self.items.push(match parse {
        //     ParseAs::Movie => self.items.push(RawCatalogItem::Movie(dir_entry)),
        //     ParseAs::Show => todo!(),
        // })

        // Map a `Result<Movie, MediaCreateError>` or `Result<Show, MediaCreateError>` to a wrapped
        // `Result<MediaItem, MediaCreateError>` to allow propagating errors while using typing system
        let res: Result<MediaItem, MediaCreateError> = match parse {
            ParseAs::Movie => Movie::new(dir_entry.path().to_path_buf())
                .map(|m| MediaItem::Movie(Arc::new(m))),
            ParseAs::Show  => Show::new(dir_entry.path().to_path_buf())
                .map(|s| MediaItem::Show(Arc::new(s))),
        };
        // process the error
        if let Err(err) = res {
            let buf = dir_entry.path().to_path_buf();
            if let Some(ref tx) = self.status_tx &&
               let Err(e) = tx.send(Ok(format!("{label}     Error during creation: {:?}", err))).await {
                tracing::info!("[CatalogBuilder::push_item] Failed to send message on channel. (destroyed?)");
                return;
            }
            self.fails.push(CatalogFailure{err, buf});
            return;
        }
        // unwrap media item and push to proper vector
        match res.unwrap() {
            MediaItem::Movie(movie) => self.movies.push(movie),
            MediaItem::Show(show) => self.shows.push(show),
            MediaItem::Episode(ep) => todo!(),
        }

    }
}
