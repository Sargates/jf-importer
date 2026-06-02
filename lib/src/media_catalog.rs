use std::{ fmt, fs, io };
use std::env;
use std::ffi::OsString;
use std::path::{ Path, PathBuf };
use std::sync::{Arc, Weak};

use regex::Regex;
use ignore::*;

use tokio::sync::{Mutex, watch};
use futures::executor::block_on;

use crate::media_item::{MediaItem, Movie, Show, Episode};
use crate::api::QueryResponse;
use crate::config::{Config,CONFIG,Secrets,SECRETS};

pub struct MediaCatalog {
    pub config: Config,
    pub movies: Vec<Arc<Mutex<Movie>>>,
    pub shows: Vec<Arc<Mutex<Show>>>,
    pub episodes: Vec<Arc<Mutex<Episode>>>,
    pub tree: TreeNode,
    what_i_am_doing: watch::Sender<String>,
}

#[derive(Debug, Clone)]
pub enum CreateError {
    FailedToCreateMediaItem,
    PathNotUnicode,
    IncorrectFileTypeSupplied,
    EpisodeIncorrectFormat,
    EpisodeFailedToParseSeason,
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
        let mut builder = WalkBuilder::new(movies_dir);
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
            self.what_i_am_doing.send(dir_entry.path().to_string_lossy().to_string()).unwrap();
            if let Err(err) = create_result {
                let fails = fails_cat.children_mut().unwrap();
                let buf = dir_entry.path().to_path_buf();
                fails.push(TreeNode::Fail{err, buf});
                continue;
            }
            let movie = create_result.unwrap();
            movies.push(Arc::new(Mutex::new(movie)));
        }
        drop(builder);

        tracing::info!("Beginning Shows Dir Walk");
        let mut builder = WalkBuilder::new(shows_dir);
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
            self.what_i_am_doing.send(dir_entry.path().to_string_lossy().to_string()).unwrap();
            if let Err(err) = create_result {
                let fails = fails_cat.children_mut().unwrap();
                let buf = dir_entry.path().to_path_buf();
                fails.push(TreeNode::Fail{err, buf});
                continue;
            }
            let show = create_result.unwrap();
            shows.push(Arc::new(Mutex::new(show)));
        }
        drop(builder);

        tracing::info!("Beginning Episodes Walk");
        for show in shows.iter_mut() {
            let mut guard = block_on(show.lock());
            self.what_i_am_doing.send(guard.src.to_string_lossy().to_string()).unwrap();
            let mut builder = WalkBuilder::new(&guard.src);
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

                let create_result = Episode::new(dir_entry.path().to_path_buf(), Arc::downgrade(show));
                self.what_i_am_doing.send(dir_entry.path().to_string_lossy().to_string()).unwrap();
                if let Err(err) = create_result {
                    let fails = fails_cat.children_mut().unwrap();
                    let buf = dir_entry.path().to_path_buf();
                    fails.push(TreeNode::Fail{err, buf});
                    continue;
                }
                let episode = create_result.unwrap();
                let arc = Arc::new(Mutex::new(episode));
                episodes.push(arc.clone()); // push to global Episodes
                guard.episodes.push(arc.clone()); // push to Show's episodes
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

#[derive(Debug)]
pub enum TreeGenError {
    EnvVarNotSet,
    EnvVarNotUnicode(OsString),
    MoviesDirDoesntExist,
    ShowsDirDoesntExist,
    RegexSyntaxError(String),
    RegexTooBig(usize),
    RegexUnknown,
}
impl From<regex::Error> for TreeGenError {
    fn from(value: regex::Error) -> Self {
        match value {
            regex::Error::Syntax(syntax) => Self::RegexSyntaxError(syntax),
            regex::Error::CompiledTooBig(size) => Self::RegexTooBig(size),
            _ => Self::RegexUnknown
        }
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
        err: CreateError,
        buf: PathBuf
    },
}
impl Into<TreeNode> for Arc<Mutex<Movie>> {
    fn into(self) -> TreeNode {
        TreeNode::Item {
            inner: MediaItem::Movie(self),
            children: vec![]
        }
    }
}
impl Into<TreeNode> for Arc<Mutex<Show>> {
    fn into(self) -> TreeNode {
        let new_children = self.try_lock().unwrap().episodes.iter().map(|ep| ep.clone().into()).collect();
        TreeNode::Item {
            inner: MediaItem::Show(self),
            children: new_children,
        }
    }
}
impl Into<TreeNode> for Arc<Mutex<Episode>> {
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
                match inner {
                    MediaItem::Show(show)       => block_on(show.lock()).query    != None,
                    MediaItem::Movie(movie)     => block_on(movie.lock()).query   != None,
                    MediaItem::Episode(episode) => block_on(episode.lock()).query != None,
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
                match inner {
                    MediaItem::Show(show) => {
                        match show.try_lock() {
                            Ok(guard) => {
                                match &guard.query {
                                    Some(response) => {
                                        write!(f, "{} ({}) ", response.title, response.year);
                                        if let Some(imdb) = &response.imdb {
                                            write!(f, "[imdbid-{}]", imdb) } 
                                        else { write!(f, "[tmdbid-{}]", response.tmdb) }
                                    }
                                    None => write!(f, "{}", guard.src.to_string_lossy()),
                                }
                            }
                            Err(err) => write!(f, "Failed to get lock for Episode! Error: {:?}", err)
                        }
                    }
                    MediaItem::Movie(movie) => {
                        match movie.try_lock() {
                            Ok(guard) => {
                                match &guard.query {
                                    Some(response) => {
                                        write!(f, "{} ({}) ", response.title, response.year);
                                        if let Some(imdb) = &response.imdb {
                                            write!(f, "[imdbid-{}]", imdb) } 
                                        else { write!(f, "[tmdbid-{}]", response.tmdb) }
                                    }
                                    None => write!(f, "{}", guard.src.to_string_lossy()),
                                }
                            }
                            Err(err) => write!(f, "Failed to get lock for Episode! Error: {:?}", err)
                        }
                    }
                    MediaItem::Episode(ep) => {
                        // I used to get deadlock if I used `block_on(ep.lock())`, but doing it this way is better 
                        // because there shouldn't be any risk of deadlocking in synchronous code.
                        match ep.try_lock() {
                            Ok(guard) => {
                                match (&guard.query, Weak::upgrade(&guard.parent).unwrap().try_lock()) {
                                    (_, Ok(parent_guard)) 
                                    if parent_guard.query.is_some() => write!(f, "{} {}", parent_guard.query.as_ref().unwrap().title.clone(), guard.id),
                                    (_, Ok(parent_guard)) 
                                    if parent_guard.query.is_none() => write!(f, "Parent query is empty: {}", guard.src.to_string_lossy()),
                                    (_, Err(err)        )               => write!(f, "Failed to get Parent Lock: {}", guard.src.to_string_lossy()),
                                    _                                   => unreachable!()
                                }
                            }
                            Err(err) => write!(f, "Failed to get lock for Episode! Error: {:?}", err)

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
