use std::{ fmt, fs, io };
use std::env;
use std::ffi::OsString;
use std::path::{ Path, PathBuf };
use std::sync::{Arc, Weak};

use regex::Regex;
use ignore::*;

use tokio::sync::{Mutex, watch};
use futures::executor::block_on;

use crate::media_item::{Episode, MediaItem, Movie, Show};
use crate::api::{QueryStatus, QueryResponse};
use crate::config::{Config,CONFIG,Secrets,SECRETS};

use crate::API_CALLS;



pub struct MediaCatalog {
    pub config: Config,
    pub movies: Vec<Arc<Movie>>,
    pub shows: Vec<Arc<Show>>,
    pub episodes: Vec<Arc<Episode>>,
    pub tree: TreeNode,
    what_i_am_doing: watch::Sender<String>,
}

#[derive(Debug, Clone)]
pub enum MediaCreateError {
    FailedToCreateMediaItem,
    PathNotUnicode,
    IncorrectFileTypeSupplied,
    EpisodeIncorrectFormat,
    EpisodeFailedToParseSeason,
}

#[derive(Debug)]
pub enum TreeGenError {
    EnvVarNotSet,
    EnvVarNotUnicode(OsString),
    MoviesDirDoesntExist,
    ShowsDirDoesntExist,
    RegexError(regex::Error),
    StatusReportSendError(watch::error::SendError<String>)
}
impl From<regex::Error> for TreeGenError {
    fn from(value: regex::Error) -> Self {
        Self::RegexError(value)
    }
}
impl From<watch::error::SendError<String>> for TreeGenError {
    fn from(value: watch::error::SendError<String>) -> Self {
        Self::StatusReportSendError(value)
    }
}

impl MediaCatalog {
    pub fn new(config: Config) -> MediaCatalog {
        let (tx, _) = watch::channel(String::from("Waiting for Feedback!"));
        MediaCatalog {
            config,
            movies: vec![],
            shows: vec![],
            episodes: vec![],
            tree: TreeNode::Category { name: format!("Uninitialized Tree"), children: vec![] },
            what_i_am_doing: tx
        }
    }

    pub fn generate_catalog_tree(mut self) -> Result<MediaCatalog, TreeGenError> {
        let cfg_base = &self.config.SrcBaseDir;
        let movies_dir = cfg_base.join(&self.config.SrcMovieSubDir);
        let shows_dir  = cfg_base.join(&self.config.SrcShowSubDir);

        tracing::info!("Start of Generation");
        if ! Path::new(&movies_dir).exists()
            { return Err(TreeGenError::MoviesDirDoesntExist); }
        if ! Path::new(&shows_dir).exists()
            { return Err(TreeGenError::ShowsDirDoesntExist); }

        // I HAVE NEVER GOTTEN TO USE CURRYING BEFORE!!! LET'S GO
        let currying_match = |re: Regex, invert: bool| 
            move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) };

        let ignore_re = Regex::new(r".*(ignore|temp).*")?;
        let video_re = Regex::new(CONFIG.clone().VideoFileExtensions.as_str())?;

        let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
        let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
        let is_match = move |x: &String, re: &Regex| re.is_match(x);


        let mut root = TreeNode::Category{ name: String::from("."), children: vec![] };
        let mut movies_cat: TreeNode = TreeNode::Category { name: String::from("Movies"), children: Vec::new()};
        let mut shows_cat:  TreeNode = TreeNode::Category { name: String::from("Shows"),  children: Vec::new()};
        let mut fails_cat:  TreeNode = TreeNode::Category { name: String::from("Failures"),  children: Vec::new()};

        let mut movies = vec![];
        let mut shows = vec![];
        let mut episodes = vec![];

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

            let create_result = Movie::new(dir_entry.path().to_path_buf());
            //? Can symlinks cause problems with the `unwrap` on `strip_prefix`?
            //? Are paths eagerly evaluated?
            // TODO: FITFO
            self.what_i_am_doing.send(format!("[Movie]   Found: {}", dir_entry.file_name().to_string_lossy().to_string()));
            if let Err(err) = create_result {
                let fails = fails_cat.children_mut().unwrap();
                let buf = dir_entry.path().to_path_buf();
                fails.push(TreeNode::Fail{err, buf});
                continue;
            }
            let movie = Arc::new(create_result.unwrap());
            API_CALLS.push_query(MediaItem::Movie(movie.clone()), QueryStatus::NotStarted);
            movies.push(movie);
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

            let create_result = Show::new(dir_entry.path().to_path_buf());
            self.what_i_am_doing.send(format!("[Show]    Found: {}", dir_entry.file_name().to_string_lossy().to_string()));
            if let Err(err) = create_result {
                let fails = fails_cat.children_mut().unwrap();
                let buf = dir_entry.path().to_path_buf();
                fails.push(TreeNode::Fail{err, buf});
                continue;
            }
            let show = Arc::new(create_result.unwrap());
            API_CALLS.push_query(MediaItem::Show(show.clone()), QueryStatus::NotStarted);
            shows.push(show);
        }
        drop(builder);

        tracing::info!("Beginning Episodes Walk");
        for show in shows.iter_mut() {
            // let mut guard = block_on(show.lock());
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
                    let fails = fails_cat.children_mut().unwrap();
                    let buf = dir_entry.path().to_path_buf();
                    fails.push(TreeNode::Fail{err, buf});
                    continue;
                }
                let episode = create_result.unwrap();
                let arc = Arc::new(episode);
                API_CALLS.push_query(MediaItem::Episode(arc.clone()), QueryStatus::NotStarted);
                episodes.push(arc.clone());
                match show.episodes.try_lock() {
                    // why the fuck have I never needed this syntax before now??
                    Ok(mut lock) => { lock.push(arc.clone()); }
                    Err(e)       => tracing::error!("Failed to acquire lock for show: {}", show.src.to_string_lossy()),

                }
            }
        }

        for movie_box in movies.iter() {
            movies_cat.push_child(movie_box.clone().into());
        }

        for show in shows.iter() {
            shows_cat.push_child(show.clone().into());
        }

        let children = root.children_mut().unwrap();
        children.push(movies_cat);
        children.push(shows_cat);
        children.push(fails_cat);

        Ok(MediaCatalog {
            config: self.config,
            movies,
            shows,
            episodes,
            tree: root,
            what_i_am_doing: self.what_i_am_doing
        })
    }
    pub fn subscribe(&self) -> watch::Receiver<String> {
        self.what_i_am_doing.subscribe()
    }
}

#[derive(Debug, Clone)]
pub enum TreeNode {
    Category {
        name: String,
        children: Vec<TreeNode>
    },
    Item {
        inner: MediaItem,
        children: Vec<TreeNode>
    },
    Fail {
        err: MediaCreateError,
        buf: PathBuf
    },
}
impl Into<TreeNode> for Arc<Movie> {
    fn into(self) -> TreeNode {
        TreeNode::Item {
            inner: MediaItem::Movie(self),
            children: vec![]
        }
    }
}
impl Into<TreeNode> for Arc<Show> {
    fn into(self) -> TreeNode {
        let new_children = self.episodes.try_lock().unwrap().iter().map(|ep| ep.clone().into()).collect();
        TreeNode::Item {
            inner: MediaItem::Show(self),
            children: new_children,
        }
    }
}
impl Into<TreeNode> for Arc<Episode> {
    fn into(self) -> TreeNode {
        TreeNode::Item {
            inner: MediaItem::Episode(self),
            children: vec![]
        }
    }
}
impl TreeNode {
    pub fn children(&self) -> Option<&Vec<TreeNode>> {
        match self {
            TreeNode::Category{ name, children } => Some(children),
            TreeNode::Item{ inner, children }    => Some(children),
            _                                    => None
        }
    }
    pub fn children_mut(&mut self) -> Option<&mut Vec<TreeNode>> {
        match self {
            TreeNode::Category{ name, children } => Some(children),
            TreeNode::Item{ inner, children }    => Some(children),
            _                                    => None
        }
        }
    pub fn push_child(&mut self, child: TreeNode) {
        match self {
            TreeNode::Category{ name, children } => { children.push(child); }
            TreeNode::Item{ inner, children }    => { children.push(child); }
            _                                    => panic!("Expected variant with children")
        }
    }
    pub fn is_queried(&self) -> bool {
        match self {
            TreeNode::Item { inner, children } => {
                match *API_CALLS.get_query(inner).unwrap() {
                    QueryStatus::NotStarted => false,
                    _                       => false
                }
            }
            _ => panic!("Expected Movie, Show, or Episode!")
        }
    }
}
impl std::fmt::Display for TreeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // writeln!(f, "Printing: {:?}", self);
        match self {
            TreeNode::Category{ name, children } => {
                write!(f, "{name}")
            }
            TreeNode::Item { inner, children } => {
                match &inner {
                    MediaItem::Show(show) => {
                        let r_status = API_CALLS.get_query(inner).unwrap();
                        match &*r_status {
                            QueryStatus::Success(response) => {
                                write!(f, "{} ({}) ", response.title, response.year);
                                if let Some(imdb) = &response.imdb {
                                    write!(f, "[imdbid-{}]", imdb) } 
                                else { write!(f, "[tmdbid-{}]", response.tmdb) }
                            }
                            _ => write!(f, "[{r_status:?}] {}", show.src.to_string_lossy()),
                        }
                    }
                    MediaItem::Movie(movie) => {
                        let r_status = API_CALLS.get_query(inner).unwrap();
                        match &*r_status {
                            QueryStatus::Success(response) => {
                                write!(f, "{} ({}) ", response.title, response.year);
                                if let Some(imdb) = &response.imdb {
                                    write!(f, "[imdbid-{}]", imdb) } 
                                else { write!(f, "[tmdbid-{}]", response.tmdb) }
                            }
                            _ => write!(f, "[{r_status:?}] {}", movie.src.to_string_lossy()),
                        }
                    }
                    MediaItem::Episode(ep) => {
                        // I used to get a deadlock if I used `block_on` directly, but doing it this way is better
                        // because there shouldn't be any risk of deadlocking in synchronous code.
                        let parent_item = MediaItem::Show(Weak::upgrade(&ep.parent).unwrap());
                        match (&*API_CALLS.get_query(&inner).unwrap(), &*API_CALLS.get_query(&parent_item).unwrap()) {

                            (_, QueryStatus::Success(response)) => {
                                write!(f, "{} ({}) ", response.title, response.year);
                                if let Some(imdb) = &response.imdb {
                                    write!(f, "[imdbid-{}]", imdb) } 
                                else { write!(f, "[tmdbid-{}]", response.tmdb) }

                                // match &*parent_guard {
                                //     QueryStatus::Success(response) => write!(f, "{} {}", response.title.clone(), ep.id),
                                //     QueryStatus::Failed(err)       => write!(f, "[Parent Failure]: {}", ep.src.to_string_lossy()),
                                //     QueryStatus::NotStarted        => write!(f, "[Parent Empty]: {}", ep.src.to_string_lossy()),
                                //     QueryStatus::InProgress        => write!(f, "Parent query is in progress: {}", ep.src.to_string_lossy()),
                                // }
                            },
                            (_, _)
                                => write!(f, "Parent query Failed: {}", ep.src.to_string_lossy()),
                            _ => unreachable!()
                        }

                    }
                }
            }
            TreeNode::Fail{ err, buf } => {
                write!(f, "Parse Failure: {buf:?}")
            }
        }
    }
}
