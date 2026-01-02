use std::error::Error;
use std::fs;
use std::fmt;
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;
use regex::Regex;
use reqwest::blocking::{Client, Request, Response};
use std::rc::{Rc, Weak};
use std::cell::RefCell;

#[derive(Debug)]
pub struct BasicError(String);
impl BasicError {
    pub fn new(msg: String) -> Self { BasicError(msg) }
    pub fn boxed(msg: String) -> Box<dyn Error> { Box::new(BasicError(msg)) }
}
impl fmt::Display for BasicError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) } }
impl Error for BasicError {}

#[macro_export]
macro_rules! err {
    ($fmt:literal, $($arg:tt)*) => { BasicError::boxed(format!($fmt, $($arg)*)) };
    ($fmt:literal)              => { BasicError::boxed(format!($fmt)) };
    ($fmt:expr)                 => { BasicError::boxed(format!($fmt)) };
}
pub(crate) use err;

pub struct Movie {
    pub title: String,
    pub year:  String,
    pub imdb:  String,
    pub src:   String
}
impl Movie {
    pub fn new() -> Self { Movie { 
        title: String::new(),
        year: String::new(),
        imdb: String::new(),
        src: String::new(), } }
}
impl fmt::Debug for Movie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({}) [{}] - ({})", self.title, self.year, self.imdb, self.src)
    }
}

pub struct Show {
    pub title:      String,
    pub start_year: String,
    pub tmdb:       String,
    pub root_dir:   String,
    pub episodes:   Vec<Episode> //? "episodes" doesn't really work for things like featurettes or extras
}
impl Show {
    pub fn new() -> Self { Show {
        title: String::new(),
        root_dir: String::new(),
        start_year: String::new(),
        tmdb: String::new(),
        episodes: Vec::new(), } }

}
impl fmt::Debug for Show {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{} ({}) [{}]", self.title, self.start_year, self.tmdb)?;
        for e in self.episodes.iter() { writeln!(f, "{:#?}", e); }
        Ok(())
    }
}

pub struct Episode {
    pub id: String,
    pub src: String,
    pub parent: Rc<RefCell<Show>>,
}
impl Episode {
    pub fn new(id: String, src: String, parent: Rc<RefCell<Show>>) -> Self {
        Episode {
            id:   String::new(),
            src:  String::new(),
            parent, } } 
}
impl fmt::Debug for Episode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} -> {}", self.id, self.src)?; 
        Ok(())
    }
}

pub struct env_t {
    pub src_dir:        String,
    pub dst_dir:        String,
    pub show_dir_name:  String,
    pub movie_dir_name: String,
    pub omdb_key:       String,
    pub tmdb_key:       String,
    pub cache_dir:      String }
impl env_t {
    pub fn new() -> Self { env_t{
        src_dir:          String::new(),
        dst_dir:          String::new(),
        show_dir_name:    String::new(),
        movie_dir_name:   String::new(),
        omdb_key:         String::new(),
        tmdb_key:         String::new(),
        cache_dir:        String::new() } } }

pub struct db_t {
    pub movies: Vec<Movie>,
    pub shows:  Vec<Rc<RefCell<Show>>>,
    pub failed_enumerations: Vec<PathBuf>, }

impl db_t {
    pub fn new() -> Self { db_t {
        movies:  Vec::new(),
        shows:   Vec::new(),
        failed_enumerations: Vec::new(), } } }

pub struct App {
    pub env: env_t,
    pub db: db_t,
    pub request_client: Client,

}

pub struct Api {}
