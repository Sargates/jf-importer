//* "query" as in querying the state of the machine
#![allow(nonstandard_style)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::error::Error;
use std::ffi::OsStr;
use std::path::Path;

use std::rc::{Rc, Weak};
use std::cell::RefCell;

use ignore::*;
use dotenv_parser::parse_dotenv;
use regex::Regex;

use crate::types::*;

const VIDEO_FILE_EXTENTIONS: &str = r"(webm|mp4|mov|mkv|m4v|avi)";

// TODO: Go to async API. remove `::blocking` from include below
use reqwest::blocking::{
    Client,
    Request, RequestBuilder,
    Response,
};
// TODO: Create a type that's returned by the `fetch_` methods when moving to async
//?  Maybe include the returned Futures

fn read_file(path: String) -> Result<String, Box<dyn Error>> {
    let res = fs::read_to_string(&path);
    if let Err(err) = res {
        return Err(err!("OS Error for file: `{}` - {}", path, err));
    }
    Ok(res.unwrap())
}
impl App {
    pub fn new() -> Self {
        App {
            env: env_t::new(),
            db: db_t::new(),
            request_client: Client::new(),
        }
    }
    pub fn load_env(mut self) -> Result<Self, Box<dyn Error>> {
        let base_env_source = read_file("base.env".to_string())?;
        let env_source = read_file(".env".to_string())?;

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
                "SRC_DIR"                 => self.env.src_dir         = v,
                "DST_DIR"                 => self.env.dst_dir         = v,
                "JELLYFIN_MOVIE_DIR_NAME" => self.env.movie_dir_name  = v,
                "JELLYFIN_SHOW_DIR_NAME"  => self.env.show_dir_name   = v,
                "OMDB_APIKEY"             => self.env.omdb_key        = v,
                "TMDB_READ_ACCESS_TOKEN"  => self.env.tmdb_key        = v,
                "CACHE_DIR"               => self.env.cache_dir       = v,
                _ => eprintln!("Unrecognized ENV Var: {}", k),
            }
        };
        Ok(self)
    }
    pub fn build_db(mut self) -> Result<Self, Box<dyn Error>> {
        // TODO: Check that these directories actually exist before running
        let movies_path = self.env.src_dir.clone() + "/movies"; // TODO: don't hardcode these
        let shows_path  = self.env.src_dir.clone() + "/shows";  // TODO: don't hardcode these
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
            m.src = movie.path().to_str().unwrap().to_string();
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
            self.db.shows.push(
                Rc::new( RefCell::new( Show {
                    title: String::new(),
                    root_dir: show.path().to_str().unwrap().to_string(),
                    tmdb: String::new(),
                    start_year: String::new(),
                    episodes: Vec::new(), })));
        }

        Ok(self)
    }
    pub fn enumerate_shows(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
        let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
        let is_match = move |x: &String, re: &Regex| re.is_match(x);
        let is_dir   = move |x: &DirEntry|           x.path().is_dir();
        let is_file  = move |x: &DirEntry|           x.path().is_file();

        for show in self.db.shows.iter_mut() {

            let mut show_mut = show.borrow_mut();
            let mut builder = WalkBuilder::new(&show_mut.root_dir);
            builder.min_depth(Some(1))
                .sort_by_file_path(|a, b| a.cmp(b)); // a < b

            for result in builder.build() {
                if is_dir(&result.clone()?) { continue; }
                let r = result?;
                let path_obj = r.path();
                let empty = OsStr::new("");
                // println!("{:#?}", &path_obj);

                if let None = path_obj.file_name().unwrap().to_str() { // invalid path; non-UTF8 (potentially extended ASCII) path name
                    self.db.failed_enumerations.push(path_obj.to_path_buf());
                    continue;
                }

                let file_path = path_obj.to_str().unwrap().to_string();

                if !is_match(&file_path, &SE_match_re) { continue; } // Not in format `SXXEXX`

                let first_match = SE_match_re.find(&file_path);
                if first_match.is_none() { continue; }
                let episode_ident = first_match.unwrap().as_str();

                let mut iterator = SE_number_re.find_iter(&episode_ident);

                if SE_number_re.find_iter(&episode_ident).count() != 2 { eprintln!("Invalid episode identifier for path: {}", &file_path); continue; }

                let season_num = iterator.next().unwrap().as_str();
                let episode_num = iterator.next().unwrap().as_str();

                let episode_string = format!("S{}E{}", season_num, episode_num);


                show_mut.episodes.push( Episode {
                    id: episode_string,
                    src: file_path,
                    parent: Rc::clone(show) } );
            }
        }
        Ok(self)
    }
}
impl Api {
    fn query_api(builder: RequestBuilder, url: String) -> reqwest::Result<Response> {
        builder.send()
    }
    pub fn query_for_movie(movie_in: &String, client: &Client, omdb_key: &String) -> reqwest::Result<Response> {
        let search_param: String;

        // Need to differentiate between searching with a raw string or by its imdb_id
        let imdb_id_re = Regex::new(r"^tt[0-9]+").unwrap(); // TODO: add error checking to make sure the regex pattern is valid

        if let Ok(res) = Api::check_override(movie_in) { search_param = res; } // set movie if hardcoded, otherwise just copy `movie_in`
        else                                           { search_param = movie_in.clone(); } 

        let url;
        if imdb_id_re.is_match(&search_param) { url = format!("https://www.omdbapi.com/?apikey={}&type=movie&i={}",  omdb_key, search_param); }
        else                                  { url = format!("https://www.omdbapi.com/?apikey={}&type=movie&s={}*", omdb_key, urlencoding::encode(&search_param)); }
        let builder = client.get(&url);
        Api::query_api(builder, url.clone())
    }
    pub fn check_override(movie: &String) -> Result<String, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string("manual-fixes.json")?;
        let json = json::parse(&contents)?;
        // eprintln!("{} {} {}", json, movie, json[movie]);
        if json[movie].is_null() { return Err(err!("Movie is not hardcoded")); }
        let id = json[movie].to_string();
        Ok(id)
    }
    pub fn query_for_show(show: &String, client: &Client, tmdb_key: &String) -> reqwest::Result<Response> {
        let encoded = urlencoding::encode(&show);
        let url = format!("https://api.themoviedb.org/3/search/tv?query={}", encoded);
        let builder = client.get(&url)
            .header("Authorization", format!("Bearer {}", tmdb_key))
            .header("accept", "application/json");

        Api::query_api(builder, url.clone())
    }
    pub fn fetch_api_shows(app: &App) -> Result<Vec<(usize, Response)>, Box<dyn std::error::Error>> {
        let client = &app.request_client;

        let mut results: Vec<(usize, Response)> = vec![];

        for (i, show) in app.db.shows.iter().enumerate() {
            let show_ref = show.borrow();
            let search_query = Path::new(&show_ref.root_dir).file_name().unwrap().to_str().unwrap().to_string();
            let result = Api::query_for_show(&search_query, &client, &app.env.tmdb_key)?;
            results.push((i, result));
        }

        Ok(results)
    }
    pub fn fetch_api_movies(app: &App) -> Result<Vec<(usize, Response)>, Box<dyn std::error::Error>> {
        let client = &app.request_client;

        let mut results: Vec<(usize, Response)> = vec![];

        for (i, movie) in app.db.movies.iter().enumerate() {
            let search_query = Path::new(&movie.src).file_stem().unwrap().to_str().unwrap().to_string();
            let result = Api::query_for_movie(&search_query, &client, &app.env.omdb_key)?;
            results.push((i, result));
        }

        Ok(results)
    }
}
