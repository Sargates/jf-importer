use std::{ fmt, fs, io };
use std::env;
use std::ffi::OsString;
use std::path::{ Path, PathBuf };
use std::sync::{Arc, Weak};

use regex::Regex;
use ignore::*;

use tokio::sync::{Mutex, watch};

use crate::media::{Episode, MediaItem, Movie, Show, Catalog, MediaCreateError};
use crate::api::{
    calls::ApiManifest,
    client::{QueryResponse, QueryStatus}
};
use crate::config::{Config,CONFIG,Secrets,SECRETS};

#[derive(Debug)]
pub enum CatalogBuildError {
    EnvVarNotSet,
    EnvVarNotUnicode(OsString),
    MoviesDirDoesntExist,
    ShowsDirDoesntExist,
    RegexError(regex::Error),
    StatusReportSendError(watch::error::SendError<String>)
}
impl From<regex::Error> for CatalogBuildError {
    fn from(value: regex::Error) -> Self { Self::RegexError(value) }
}
impl From<watch::error::SendError<String>> for CatalogBuildError {
    fn from(value: watch::error::SendError<String>) -> Self { Self::StatusReportSendError(value) }
}

pub struct CatalogBuilder {
    config: Config,
    what_i_am_doing: watch::Sender<String>,

    movies: Vec<Arc<Movie>>,
    shows: Vec<Arc<Show>>,
    episodes: Vec<Arc<Episode>>,
    fails: Vec<CatalogFailure>,
}

pub struct CatalogFailure {
    pub err: MediaCreateError,
    pub buf: PathBuf
}

impl CatalogBuilder {
    pub fn new(config: Config) -> Self {
        let (tx, _) = watch::channel(String::from("Waiting for Feedback!"));
        Self {
            config,
            what_i_am_doing: tx,
            movies: Vec::new(),
            shows: Vec::new(),
            episodes: Vec::new(),
            fails: Vec::new(),
        }
    }
    pub fn build(mut self) -> Result<Catalog, CatalogBuildError> {
        let cfg_base = &self.config.SrcBaseDir;
        let movies_dir = cfg_base.join(&self.config.SrcMovieSubDir);
        let shows_dir  = cfg_base.join(&self.config.SrcShowSubDir);

        tracing::info!("Building Catalog!");
        if ! Path::new(&movies_dir).exists()
            { return Err(CatalogBuildError::MoviesDirDoesntExist); }
        if ! Path::new(&shows_dir).exists()
            { return Err(CatalogBuildError::ShowsDirDoesntExist); }

        // I HAVE NEVER GOTTEN TO USE CURRYING BEFORE!!! LET'S GO
        // can't use shared reference here because we'd need lifetime syntax 
        // and that can't be enabled with current rust version
        let currying_match = |re: Regex, invert: bool| 
            move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) };

        let ignore_re = Regex::new(r".*(ignore|temp).*")?;
        let video_re = Regex::new(CONFIG.clone().VideoFileExtensions.as_str())?;

        let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
        let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
        let is_match = |x: &String, re: &Regex| re.is_match(x);

        tracing::info!("Beginning Movies Dir Walk");
        let mut builder = WalkBuilder::new(movies_dir.clone());
        builder.min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(currying_match(ignore_re.clone(), true))
            .filter_entry(currying_match(video_re.clone(), false));
        for result in builder.build() {
            if let Err(err) = result {
                tracing::error!("Bad Dir Entry {}", err);
                continue;
            }
            let dir_entry = result.unwrap();
            if ! dir_entry.path().is_file() { continue; }
            self.push_movie(dir_entry);
        }
        drop(builder);

        tracing::info!("Beginning Shows Dir Walk");
        let mut builder = WalkBuilder::new(shows_dir.clone());
        builder.min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(currying_match(ignore_re.clone(), true));
        for result in builder.build() {
            if let Err(err) = result {
                tracing::error!("Bad Dir Entry {}", err);
                continue;
            }
            let dir_entry = result.unwrap();
            if ! dir_entry.path().is_dir() { continue; }
            self.push_show(dir_entry);
        }
        drop(builder);

        tracing::info!("Beginning Episodes Walk");
        for show in self.shows.iter_mut() {
            self.what_i_am_doing.send(format!("[Show]    Looking for Episodes: {}", show.src.strip_prefix(shows_dir.clone()).unwrap().to_string_lossy().to_string()));
            let mut builder = WalkBuilder::new(&show.src);
            builder.min_depth(Some(1))
                //* uncommenting these causes crashes
                // .filter_entry(currying_match(ignore_re.clone(), true))
                // .filter_entry(currying_match(video_re.clone(), false))
                .sort_by_file_path(|a, b| a.cmp(b)); // a < b
            for result in builder.build() {
                if let Err(err) = result {
                    tracing::error!("Bad Dir Entry {}", err);
                    continue;
                }
                let dir_entry = result.unwrap();
                if ! dir_entry.path().is_file() { continue; } // skip directories

                let create_result = Episode::new(dir_entry.path().to_path_buf(), Arc::downgrade(&*show));
                self.what_i_am_doing.send(format!("[Episode] Found: {}", dir_entry.path().strip_prefix(shows_dir.clone()).unwrap().to_string_lossy().to_string()));
                if let Err(err) = create_result {
                    let buf = dir_entry.path().to_path_buf();
                    self.fails.push(CatalogFailure{err, buf});
                    continue;
                }
                let episode = create_result.unwrap();
                let arc = Arc::new(episode);
                // API_CALLS.push_query(MediaItem::Episode(arc.clone()), QueryStatus::NotStarted);
                self.episodes.push(arc.clone());
                match show.episodes.try_lock() {
                    // why the fuck have I never needed this syntax before now??
                    Ok(mut lock) => { lock.push(arc.clone()); }
                    Err(e)       => tracing::error!("Failed to acquire lock for show: {}", show.src.to_string_lossy()),

                }
            }
        }

        Ok(Catalog {
            config: self.config,
            movies: self.movies,
            shows: self.shows,
            episodes: self.episodes,
            failures: self.fails,
        })
    }
    pub fn push_movie(&mut self, dir_entry: DirEntry) {
        let create_result = Movie::new(dir_entry.path().to_path_buf());
        self.what_i_am_doing.send(format!("[Movie]   Found: {}", dir_entry.file_name().to_string_lossy().to_string()));
        if let Err(err) = create_result {
            let buf = dir_entry.path().to_path_buf();
            self.fails.push(CatalogFailure{err, buf});
            return;
        }
        let movie = Arc::new(create_result.unwrap());
        // API_CALLS.push_query(MediaItem::Movie(movie.clone()), QueryStatus::NotStarted);
        self.movies.push(movie);
    }
    pub fn push_show(&mut self, dir_entry: DirEntry) {
        let create_result = Show::new(dir_entry.path().to_path_buf());
        //? Can symlinks cause problems with the `unwrap` on `strip_prefix`?
        //? Are paths eagerly evaluated?
        // TODO: FITFO
        self.what_i_am_doing.send(format!("[Show]    Found: {}", dir_entry.file_name().to_string_lossy().to_string()));
        if let Err(err) = create_result {
            let buf = dir_entry.path().to_path_buf();
            self.fails.push(CatalogFailure{err, buf});
            return;
        }
        let show = Arc::new(create_result.unwrap());
        // API_CALLS.push_query(MediaItem::Show(show.clone()), QueryStatus::NotStarted);
        self.shows.push(show);
    }
    pub fn subscribe(&self) -> watch::Receiver<String> {
        self.what_i_am_doing.subscribe()
    }
}
