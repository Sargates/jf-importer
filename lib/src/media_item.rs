use std::fmt;
use std::hash::Hash;
use std::sync::{Arc,Weak};
use std::path::{Path, PathBuf};

use tokio::sync::Mutex;
use futures::executor::block_on;

use regex::Regex;

use crate::api::{QueryStatus, QueryResponse};

use crate::media_catalog::{TreeNode, MediaCreateError};
#[derive(Debug, Clone)]
pub enum MediaItem {
    Movie(Arc<Movie>),
    Show(Arc<Show>),
    Episode(Arc<Episode>),
}
impl Hash for MediaItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            MediaItem::Movie(arc)   => arc.src.hash(state),
            MediaItem::Show(arc)    => arc.src.hash(state),
            MediaItem::Episode(arc) => arc.src.hash(state),
        }
    }
}
impl PartialEq for MediaItem {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Movie(l), Self::Movie(r))     => Arc::ptr_eq(l, r),
            (Self::Show(l), Self::Show(r))       => Arc::ptr_eq(l, r),
            (Self::Episode(l), Self::Episode(r)) => Arc::ptr_eq(l, r),
            _ => false,
        }
    }
}
impl Eq for MediaItem {}

#[derive(Debug)]
pub struct Movie {
    pub src: PathBuf,
    pub query: Mutex<QueryStatus>,
}
impl Movie {
    // TODO: How does this work for testing? How do we create dummy movies/episodes for testing?
    pub(crate) fn new(path: PathBuf) -> Result<Self, MediaCreateError> {
        let opt = path.to_str();
        if let None = opt   { return Err(MediaCreateError::PathNotUnicode); }
        if ! path.is_file() { return Err(MediaCreateError::IncorrectFileTypeSupplied); }
        let src = path;
        let query = Mutex::new(QueryStatus::NotStarted);
        Ok(Movie{ src, query })
    }
}

#[derive(Debug)]
pub struct Show {
    pub src: PathBuf, // directory containing show
    pub query: Mutex<QueryStatus>,
    pub episodes: Mutex<Vec<Arc<Episode>>>,
}
impl Show {
    /// Assumes `movies_dir` exists and is structured correctly
    /// Benefit of doing it this way is that tests are easier to write
    pub(crate) fn new(path: PathBuf) -> Result<Self, MediaCreateError> {
        let opt = path.to_str();
        if let None = opt  { return Err(MediaCreateError::PathNotUnicode); }
        if ! path.is_dir() { return Err(MediaCreateError::IncorrectFileTypeSupplied) }
        let src = path;
        let query = Mutex::new(QueryStatus::NotStarted);
        let episodes = Mutex::new(vec![]);
        Ok(Show{ src, query, episodes })
    }
}

#[derive(Debug, Clone)]
pub enum EpisodeId {
    Traditional { season: u32, episode: u32, },
    Anime { episode: u32, }
}
impl std::fmt::Display for EpisodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Into::<String>::into(self.clone()))
    }
}
impl Into<String> for EpisodeId {
    fn into(self) -> String {
        match self {
            EpisodeId::Traditional { season, episode } => format!("S{:02}E{:02}", season, episode),
            EpisodeId::Anime { episode }               => format!("{}", episode),
        }
    }
}
#[derive(Debug)]
pub struct Episode {
    pub src: PathBuf,
    pub id: EpisodeId,
    pub parent: Weak<Show>,
    pub query: Mutex<QueryStatus>,
}
impl Episode {
    // Most of this is grandfathered from pre-refactor. This code may be shit
    pub(crate) fn new(path: PathBuf, parent: Weak<Show>) -> Result<Self, MediaCreateError> {
        let opt = path.to_str();
        if let None = opt   { return Err(MediaCreateError::PathNotUnicode); }
        if ! path.is_file() { return Err(MediaCreateError::IncorrectFileTypeSupplied); }

        // We know these are good
        let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+").unwrap();
        let SE_number_re = Regex::new(r"[0-9]+").unwrap(); // multipurpose regex for season and episode number
        let is_match = move |x: &String, re: &Regex| re.is_match(x);

        let file_path = path.to_str().unwrap().to_string();

        // TODO: Support Anime numbering
        if !SE_match_re.is_match(&file_path) { return Err(MediaCreateError::EpisodeIncorrectFormat); }

        let first_match = SE_match_re.find(&file_path);
        if first_match.is_none() { return Err(MediaCreateError::EpisodeIncorrectFormat); } // just in case, too lazy to scour docs
        let episode_ident = first_match.unwrap().as_str();

        if SE_number_re.find_iter(&episode_ident).count() != 2 {
            return Err(MediaCreateError::EpisodeIncorrectFormat);
        }

        let mut iterator = SE_number_re.find_iter(&episode_ident);
        let season = iterator.next().unwrap().as_str().to_string().parse::<u32>().unwrap();
        let episode = iterator.next().unwrap().as_str().to_string().parse::<u32>().unwrap();

        let src = path;
        let id = EpisodeId::Traditional { season, episode };
        let query = Mutex::new(QueryStatus::NotStarted);

        Ok(Episode{ src, id, query, parent })
    }
}

pub trait Movable {
    type Error;
    /// Move the given MediaItem to the target location defined the configuration:
    /// - `DstBaseDir`/`JfMovieDir`/`Movable::get_mapped_path`
    ///   OR
    /// - `DstBaseDir`/`JfShowsDir`/`Movable::get_mapped_path`
    // `move` wave taken :/
    fn relocate(&self) -> Result<(),Self::Error>;
    fn get_mapped_path(&self) -> Result<PathBuf,Self::Error>;
}

// #[derive(Debug, Clone)]
// pub enum MappingError {
//     FailedToMapPath
// }

// pub trait Mappable {
//     type Error;
//     fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<PathBuf,Self::Error>;
// }

// impl Mappable for Movie {
//     type Error = MappingError;
//     fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<PathBuf,Self::Error> {
//         todo!()
//     }
// }
//
// impl Mappable for Episode {
//     type Error = MappingError;
//     fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<PathBuf,Self::Error> {
//         todo!()
//     }
// }
