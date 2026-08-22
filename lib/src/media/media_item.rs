use std::fmt;
use std::hash::Hash;
use std::sync::{Arc,Weak};
use std::path::{Path, PathBuf};

use serde_json::to_string;
use tokio::sync::Mutex;
use futures::executor::block_on;

use regex::Regex;

use crate::api::client::{QueryStatus, QueryResponse};

// use crate::media::tree::TreeNode;


#[derive(Debug, Clone)]
pub enum MediaCreateError {
    FailedToCreateMediaItem,
    PathNotUnicode,
    IncorrectFileTypeSupplied,
    EpisodeIncorrectFormat,
    EpisodeFailedToParseSeason,
}

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
impl MediaItem {
    pub fn raw_media_label(&self) -> String {
        match self {
            MediaItem::Movie(movie) => { movie.raw_media_label() }
            MediaItem::Show(show)   => { show.raw_media_label() }
            MediaItem::Episode(ep)  => { ep.raw_media_label() }
        }
    }
}

// To comment on the robustness of the following functions, These two functions were 
// able to handle all but 4 of my catalog of 361 movies and TV box 
// sets. And the 4 that failed were too generic or too specific to 
// find the correct result from the API anyway; they would need to 
// be manually tuned.

/// Auto trim file/dir names into searchable alternatives.
/// Not very robust, just more than nothing. Use a user-controlled alternative.
/// # Examples: 
/// - `Mission: Impossible III - [dvd]`
///   -> `"Mission: Impossible III"`
/// - `Pirates of the Caribbean - At World's End (2007) [tt0449088] [Bluray-1080p]`
///   -> `"Pirates of the Caribbean - At World's End"`
fn auto_trim_search_term<'a>(term: &'a str) -> String {
    let tag_trim = Regex::new(r"( *\[.*?\] *| *\(.*?\) *)").unwrap(); // jellyfin tags
    let decoration_trim = Regex::new(r"- *$").unwrap();               // leftover hyphen decorations
    let term: String = tag_trim.split(&term).collect();
    let term: String = decoration_trim.split(&term).collect();
    let term = term.trim().to_string();                               // trim
    term
}

/// Parse JF-supported tags from file names
/// Much like `auto_trim_search_term`, not robust. 
/// Nothing will beat user-controlled tagging.
pub fn tag_extract<'a>(term: &'a str) -> Vec<String> {
    let year_re = Regex::new(r"\([0-9]+?\)").unwrap();
    let id_re   = Regex::new(r"(\[[a-z]+?id-[a-z]*[0-9]+?\]|\[[a-z]*[0-9]+\])").unwrap(); 
    let tag_re  = Regex::new(r"\[.*?\]").unwrap();

    // remove all year identifiers that may be embedded in the file name
    let minus_year: String = year_re.split(term).collect();
    // remove ID. only want to remove the first instance to avoid deleting tags
    let minus_year_and_id: String = match id_re.find(&minus_year) {
        Some(m) => {
            let (left, garb_and_right) = minus_year.split_at(m.start());
            let (garb, right) = garb_and_right.split_at(m.len());
            let out = left.to_string() + right;
            out
        }
        None => minus_year.clone()
    };

    let tags_iter: Vec<String> = tag_re.find_iter(&minus_year_and_id)
        .map(|m| {
            // trim first and last character of tag since match will contain `[]` 
            // and regex crate doesn't support lookahead/lookbehind
            let mut chars = m.as_str().chars();
            chars.next();
            chars.next_back();
            chars.as_str().to_string()
        })
        .collect();

    tags_iter
}

#[derive(Debug)]
pub struct Movie {
    pub src: PathBuf,
    pub search_term: String,
    pub tags: Vec<String>,
}
impl Movie {
    /// It is expected that `path` exists and is a **file**.
    pub fn new(path: PathBuf) -> Result<Self, MediaCreateError> {
        let opt = path.to_str();
        if let None = opt   { return Err(MediaCreateError::PathNotUnicode); }
        // if ! path.is_file() { return Err(MediaCreateError::IncorrectFileTypeSupplied); }

        let stem: &str = &path.file_stem()
            .ok_or(MediaCreateError::IncorrectFileTypeSupplied)?
            .to_string_lossy();

        let search_term = auto_trim_search_term(stem);
        let tags = tag_extract(stem);

        let src = path;
        Ok(Movie{ src, search_term, tags })
    }
    pub fn raw_media_label(&self) -> String {
        self.search_term.clone()
        // self.src.file_stem().unwrap().to_string_lossy().to_string()
    }
}

#[derive(Debug)]
pub struct Show {
    pub src: PathBuf, // directory containing show
    pub episodes: Mutex<Vec<Arc<Episode>>>,
    pub search_term: String,
}
impl Show {
    /// It is expected that `path` exists and is a **directory**.
    pub(crate) fn new(path: PathBuf) -> Result<Self, MediaCreateError> {
        let opt = path.to_str();
        if let None = opt  { return Err(MediaCreateError::PathNotUnicode); }
        // if ! path.is_dir() { return Err(MediaCreateError::IncorrectFileTypeSupplied) }

        let stem: &str = &path.file_stem()
            .ok_or(MediaCreateError::IncorrectFileTypeSupplied)?
            .to_string_lossy();
        let search_term = auto_trim_search_term(stem);
        // let tags = tag_extract(stem);

        let src = path;
        let episodes = Mutex::new(vec![]);
        Ok(Show{ src, episodes, search_term })
    }
    pub fn raw_media_label(&self) -> String {
        self.src.file_name().unwrap().to_string_lossy().to_string()
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

        Ok(Episode{ src, id, parent })
    }
    pub fn raw_media_label(&self) -> String {
        self.id.to_string()
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
