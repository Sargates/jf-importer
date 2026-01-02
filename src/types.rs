use std::{fmt, fs};
use std::ffi::OsStr;
use regex::Regex;
use ignore::{*};
use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path,PathBuf};

use dotenv_parser::parse_dotenv;

const VIDEO_FILE_EXTENTIONS: &str = r"(webm|mp4|mov|mkv|m4v|avi)";

#[derive(Debug)]
pub struct BasicError(String);
impl BasicError {
    pub fn new(msg: String) -> Self { BasicError(msg) }
    pub fn boxed(msg: String) -> Box<dyn Error> { Box::new(BasicError(msg)) }
}
impl std::fmt::Display for BasicError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) } }
impl Error for BasicError {}

#[macro_export]
macro_rules! err {
    ($fmt:literal, $($arg:tt)*) => { BasicError::boxed(format!($fmt, $($arg)*)) };
    ($fmt:literal)              => { BasicError::boxed(format!($fmt)) };
    ($fmt:expr)                 => { BasicError::boxed(format!($fmt)) };
}
pub(crate) use err;


pub struct Movie {
    pub name: String,
    pub path: String,
    pub year: String,
    pub imdb: String,
}
impl Movie {
    pub fn new() -> Self { Movie { 
        name: String::new(),
        path: String::new(),
        year: String::new(),
        imdb: String::new() } }
}
impl fmt::Debug for Movie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({}) [{}] - ({})", self.name, self.year, self.imdb, self.path)
    }
}

pub struct Show {
    pub name: String,
    pub root_dir: String,
    pub year: String,
    pub tmdb: String,
    pub episodes: BTreeMap<String, String>, // "SXXEXX" -> "/path/to/episode"
}
impl Show {
    pub fn new() -> Self { Show {
        name: String::new(),
        root_dir: String::new(),
        year: String::new(),
        tmdb: String::new(),
        episodes: BTreeMap::new() } }
}
impl fmt::Debug for Show {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{} ({}) [{}]", self.name, self.year, self.tmdb)?;
        for (episode, path) in self.episodes.iter() { writeln!(f, "{} -> {}", episode, path)?; }
        Ok(())
    }
}

pub struct env_t {
    pub src_dir:   String,
    pub dst_dir:   String,
    pub omdb_key:  String,
    pub tmdb_key:  String,
    pub cache_dir: String
}
impl env_t {
    pub fn new() -> Self { env_t{
        src_dir:   String::new(),
        dst_dir:   String::new(),
        omdb_key:  String::new(),
        tmdb_key:  String::new(),
        cache_dir: String::new() } }
}

pub struct db_t {
    pub movies:  Vec<Movie>,
    pub shows:   Vec<Show>,
    pub failed_enumerations: Vec<PathBuf>
}
impl db_t {
    pub fn new() -> Self { db_t {
        movies:              Vec::new(),
        shows:               Vec::new(), 
        failed_enumerations: Vec::new() } }
}

pub struct App {
    pub env: env_t,
    pub db: db_t,
}

impl App {
    pub fn new() -> Self {
        App{ 
            env: env_t::new(),
            db:  db_t::new()
        }
    }
    pub fn load_env(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        let base_env_source = fs::read_to_string("base.env")?;
        let env_source = fs::read_to_string(".env")?;

        // I have no fucking idea if this is how you're supposed to do it. It doesn't make sense why we're 
        // adding a String and a &str type especially when Rust is extremely stingy about ownership. This language is weird
        // I am not downloading another dependency just to concatenate strings
        let full_env_source = base_env_source + "\n" + &env_source;

        // cannot get try operator to work because parse_dotenv has a strange error type, unwrap_or is safer anyway
        let parse_result = parse_dotenv(&full_env_source).unwrap_or(BTreeMap::new());

        // // let mut env = ENV.lock()?;

        println!("{:#?}", parse_result);

        for (k, v) in parse_result {
            match k.as_str() {
                "SRC_DIR"                => self.env.src_dir   = v,
                "DST_DIR"                => self.env.dst_dir   = v,
                "OMDB_APIKEY"            => self.env.omdb_key  = v,
                "TMDB_READ_ACCESS_TOKEN" => self.env.tmdb_key  = v,
                "CACHE_DIR"              => self.env.cache_dir = v,
                _ => eprintln!("Unrecognized ENV Var: {}", k),
            }
        };
        Ok(self)
    }
    pub fn construct_db(mut self) -> Result<Self, Box<dyn Error>> {
        let movies_path = self.env.src_dir.clone() + "/movies";
        let shows_path  = self.env.src_dir.clone() + "/shows";
        let currying_match = |re: Regex, invert: bool| move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) }; // I HAVE NEVER GOTTEN TO USE CURRYING BEFORE!!! LET'S GO
        let ignore_re = Regex::new(r".*(ignore|temp).*")?;
        let video_re = Regex::new(VIDEO_FILE_EXTENTIONS)?;

        let mut builder = WalkBuilder::new(movies_path);
        builder.min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(currying_match(ignore_re.clone(), true))
            .filter_entry(currying_match(video_re.clone(), false));
        for result in builder.build() {
            let movie = result.unwrap();
            if ! movie.path().is_file() { continue; }
            let mut m = Movie::new();
            m.path = movie.path().to_str().unwrap().to_string();
            self.db.movies.push(m);
        }
        drop(builder);

        let mut builder = WalkBuilder::new(shows_path);
        builder.min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(currying_match(ignore_re.clone(), true));

        for result in builder.build() {
            let show = result.unwrap();
            if ! show.path().is_dir() { continue; }
            self.db.shows.push(Show {
                name: String::new(),
                root_dir: show.path().to_str().unwrap().to_string(),
                tmdb: String::new(),
                year: String::new(),
                episodes: BTreeMap::new()
            });
        }

        Ok(self)
    }
    pub fn populate_shows(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
        let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
        let is_match = move |x: &String, re: &Regex| re.is_match(x);
        let is_dir   = move |x: &DirEntry|           x.path().is_dir();
        let is_file  = move |x: &DirEntry|           x.path().is_file();
        for show in self.db.shows.iter_mut() {
            let mut builder = WalkBuilder::new(&show.root_dir);
            builder.min_depth(Some(1))
                .sort_by_file_path(|a, b| a.cmp(b)); // a < b

            for result in builder.build() {
                if is_dir(&result.clone()?) { continue; }
                let r = result?;
                let path_obj = r.path();
                let empty = OsStr::new("");
                println!("{:#?}", &path_obj);

                if let None = path_obj.file_name() { // invalid path; non-UTF8 (potentially extended ASCII) path name
                    self.db.failed_enumerations.push(path_obj.to_path_buf());
                    continue;
                }

                let file_name = path_obj.file_name().unwrap().to_str().unwrap().to_string();

                let path = &path_obj.as_os_str().to_string_lossy().to_string();

                if !is_match(&file_name, &SE_match_re) { continue; } // Not in format `SXXEXX`

                let first_match = SE_match_re.find(&file_name);
                if first_match.is_none() { continue; }
                let episode_ident = first_match.unwrap().as_str();

                let mut iterator = SE_number_re.find_iter(&episode_ident);

                if SE_number_re.find_iter(&episode_ident).count() != 2 { eprintln!("Invalid episode identifier for path: {}", &path); continue; }

                let season_num = iterator.next().unwrap().as_str();
                let episode_num = iterator.next().unwrap().as_str();

                let episode_string = format!("S{}E{}", season_num, episode_num);

                show.episodes.insert(episode_string.clone(), path.clone());
            }
        }
        Ok(self)
    }
}
