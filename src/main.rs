#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::fmt;
use std::fmt::Display;
use std::ffi::OsStr;

use std::path::Path;

use std::collections::HashMap;
use std::collections::BTreeMap;
use json::stringify_pretty;
use lazy_static::*;

use std::string::String;
use std::string::ParseError;
use std::sync::Mutex;
use futures::{future, Future};
use futures::executor::block_on;
use json;

use regex::Regex;
use reqwest::blocking::{Response, Request};
use urlencoding;

use std::error::Error;

use std::fs;
use ignore::{*};

use dotenv_parser::parse_dotenv;
use tokio;



#[derive(Debug)]
struct BasicError(String);
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

/*
// To sort out from shell script

// Movie Options
zparseopts -D -E -F -- \
   {h,-help}=help       \
   {m,-map-filenames-to-titles}=map_names \
   {L,-add-label}:=add_label \
   {i,-ignore-catalogue}:=ignore_catalogue \
   {q,-only-query-api}=only_query \
   {p,-print-subtitles}=print_subs \
   {d,-dryrun}=should_dryrun \
   || return

// TV options
zparseopts -D -E -F -- \                    
    {h,-help}=help       \                  
    {m,-map-filenames-to-titles}=map_names \
    {L,-add-label}:=add_label \             
    {p,-print-subtitles}=print_subtitles \  
    {d,-dryrun}=should_dryrun \             
    || return                               

*/

const VIDEO_FILE_EXTENTIONS: &str = r"(webm|mp4|mov|mkv|m4v|avi)";

pub struct Movie {
    pub name: String,
    pub path: String,
    pub year: String,
    pub imdb: String,
}
impl Movie {
    fn new() -> Self { Movie{ 
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
    fn new() -> Self { Show{ name: String::new(),
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
    fn new() -> Self { env_t{
        src_dir:   String::new(),
        dst_dir:   String::new(),
        omdb_key:  String::new(),
        tmdb_key:  String::new(),
        cache_dir: String::new() } }
}

pub struct db_t {
    movies:  Vec<Movie>,
    shows:   Vec<Show>
}
impl db_t {
    fn new() -> Self { db_t {
        movies:  Vec::new(),
        shows:   Vec::new() } }
}

lazy_static! {
    pub static ref ENV: Mutex<env_t> = Mutex::new(env_t::new());
    pub static ref DB: Mutex<db_t> = Mutex::new(db_t::new());
}

fn load_env() -> Result<(), Box<dyn std::error::Error>> {
    let base_env_source = fs::read_to_string("base.env")?;
    let env_source = fs::read_to_string(".env")?;

    // I have no fucking idea if this is how you're supposed to do it. It doesn't make sense why we're 
    // adding a String and a &str type especially when Rust is extremely stingy about ownership. This language is weird
    // I am not downloading another dependency just to concatenate strings
    let full_env_source = base_env_source + "\n" + &env_source;

    // cannot get try operator to work because parse_dotenv has a strange error type
    let parse_result = parse_dotenv(&full_env_source).unwrap_or(BTreeMap::new());

    let mut env = ENV.lock()?;

    println!("{:#?}", parse_result);

    for (k, v) in parse_result {
        match k.as_str() {
            "SRC_DIR"     => env.src_dir   = v,
            "DST_DIR"     => env.dst_dir   = v,
            "OMDB_APIKEY" => env.omdb_key  = v,
            "TMDB_READ_ACCESS_TOKEN" => env.tmdb_key  = v,
            "CACHE_DIR"   => env.cache_dir = v,
            _ => eprintln!("Unrecognized ENV Var: {}", k),
        }
    };
    Ok(())
}

fn construct_db() -> Result<(), Box<dyn Error>> {
    let env = ENV.lock()?;
    let mut db = DB.lock()?;
    let movies_path = env.src_dir.clone() + "/movies";
    let shows_path  = env.src_dir.clone() + "/shows";
    let currying_match = |re: Regex, invert: bool| move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) }; // I HAVE NEVER GOTTEN TO ACTUALLY USE CURRYING BEFORE!!! LET'S GO
    let ignore_re = Regex::new(r".*(ignore|temp).*")?;
    let video_re = Regex::new(VIDEO_FILE_EXTENTIONS)?;
    

    // println!("{:?}", "Movies");
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
        db.movies.push(m);
    }
    drop(builder);

    // println!("{:?}", "Shows");
    let mut builder = WalkBuilder::new(shows_path);
    builder.min_depth(Some(1))
           .max_depth(Some(1))
           .filter_entry(currying_match(ignore_re.clone(), true));

    for result in builder.build() {
        let show = result.unwrap();
        if ! show.path().is_dir() { continue; }
        db.shows.push(Show {
            name: String::new(),
            root_dir: show.path().to_str().unwrap().to_string(),
            tmdb: String::new(),
            year: String::new(),
            episodes: BTreeMap::new()
        });
        // println!("    {:?}", show.path().display());
    }

    Ok(())
}

fn populate_shows() -> Result<(), Box<dyn std::error::Error>> {
    let env = ENV.lock()?;
    let mut db = DB.lock()?;
    let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
    let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
    let is_match = move |x: &String, re: &Regex| re.is_match(x);
    let is_dir   = move |x: &DirEntry|           x.path().is_dir();
    let is_file  = move |x: &DirEntry|           x.path().is_file();
    for show in db.shows.iter_mut() {
        let mut builder = WalkBuilder::new(&show.root_dir);
        builder.min_depth(Some(1))
                .sort_by_file_path(|a, b| a.cmp(b));

        for result in builder.build() {
            if is_dir(&result.clone()?) { continue; }
            let file = result?;
            let empty = OsStr::new("");
            let file_name = file.path().file_name().unwrap_or(empty).to_string_lossy().to_string();
            let path = &file.path().as_os_str().to_string_lossy().to_string();

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
    Ok(())
}


fn query_api(builder: reqwest::blocking::RequestBuilder, url: String) -> Result<Response, reqwest::Error> { builder.send() }
fn query_for_movie(movie_in: &String, client: &reqwest::blocking::Client, env: &env_t) -> Result<Response, reqwest::Error> {
    let searched_query: String;

    // Need to differentiate between searching with a raw string or by its imdb_id
    let imdb_id_re = Regex::new(r"^tt[0-9]+").unwrap(); // TODO when the fuck does this fail?


    if let Ok(res) = check_override(movie_in) { searched_query = res; } // set movie if hardcoded, otherwise just copy movie_in
    else                                      { searched_query = movie_in.clone(); } 

    let url;
    if imdb_id_re.is_match(&searched_query) { url = format!("http://www.omdbapi.com/?apikey={}&type=movie&i={}", env.omdb_key, searched_query); }
    else                                    { url = format!("https://www.omdbapi.com/?apikey={}&type=movie&s={}*", env.omdb_key, urlencoding::encode(&searched_query)); }
    // println!("[DEBUG]  Movie URL: {}", url);
    let builder = client.clone().get(&url);
    query_api(builder, url.clone())
}

fn check_override(movie: &String) -> Result<String, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string("manual-fixes.json")?;
    let json = json::parse(&contents)?;
    // eprintln!("{} {} {}", json, movie, json[movie]);
    if json[movie].is_null() { return Err(err!("Movie is not hardcoded")); }
    let id = json[movie].to_string();
    Ok(id)
}

fn query_for_show(show: &String, client: &reqwest::blocking::Client, env: &env_t) -> Result<Response, reqwest::Error> {
    let encoded = urlencoding::encode(&show);
    let url = format!("https://api.themoviedb.org/3/search/tv?query={}", encoded);
    let builder = client.clone().get(&url)
        .header("Authorization", format!("Bearer {}", env.tmdb_key))
        .header("accept", "application/json");

    query_api(builder, url.clone())
}

fn fetch_api_shows(env_lock: &env_t, db_lock: &db_t) -> Result<Vec<(usize, Response)>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();

    let mut results: Vec<(usize, Response)> = vec![];

    for (i, show) in db_lock.shows.iter().enumerate() {
        let search_query = Path::new(&show.root_dir).file_name().unwrap().to_str().unwrap().to_string();
        let result = query_for_show(&search_query, &client, &env_lock)?;
        results.push((i, result));
    }

    Ok(results)
}

fn fetch_api_movies(env_lock: &env_t, db_lock: &db_t) -> Result<Vec<(usize, Response)>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();

    let mut results: Vec<(usize, Response)> = vec![];

    for (i, movie) in db_lock.movies.iter().enumerate() {
        let search_query = Path::new(&movie.path).file_stem().unwrap().to_str().unwrap().to_string();
        let result = query_for_movie(&search_query, &client, &env_lock)?;
        results.push((i, result));
    }

    Ok(results)
}

fn api_stuff() -> Result<Vec<Box<dyn Error>>, Box<dyn std::error::Error>> {
    let env = ENV.lock()?;
    let mut db  = DB.lock()?;
    let responses_shows = fetch_api_shows(&env, &db)?;
    let responses_movies = fetch_api_movies(&env, &db)?;
    let year_re = Regex::new(r"^[0-9]+")?;

    let mut results: Vec<Box<dyn Error>> = vec![];
    
    if responses_shows.iter().count() == 0 {
        results.push(err!("Got no responses for shows from TMDB API"));
    }
    for (i, response) in responses_shows {
        let opt = db.shows.get_mut(i);
        if opt.is_none() { results.push(err!("No show object at element {}", i));
                           continue; }
        let show = opt.unwrap();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["total_results"].as_u64().unwrap() == 0 {
            results.push(err!("API Failure for show: {:#?}", Path::new(&show.root_dir).file_name().unwrap()));
            continue; }

        let json = &raw_json["results"][0];
        show.name = json["name"].to_string();
        show.tmdb = json["id"].to_string();
        let year = json["first_air_date"].to_string();
        show.year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid

        println!("{:#?}", show);

    }
    if responses_movies.iter().count() == 0 {
        results.push(err!("Got no responses for movies from OMDB API"));
    }

    for (i, response) in responses_movies {
        let opt = db.movies.get_mut(i);
        if opt.is_none() { results.push(err!("No show object at element {}", i));
                           continue; }
        let movie = opt.unwrap();

        
        let search_term = Path::new(&movie.path).file_stem().unwrap().to_str().unwrap().to_string();
        let hardcoded: bool;
        if let Ok(res) = check_override(&search_term) { hardcoded = true; }
        else                                          { hardcoded = false; }

        let final_url = response.url().to_string().clone();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["Response"] == "False" { results.push(err!("API Failure for movie: {:#?}", &search_term));
                                             continue; }

        let json;
        if hardcoded { json = &raw_json;              } // response changes if we search directly w/ an imdb id or a string
        else         { json = &raw_json["Search"][0]; }

        // println!("{:#?}\n{:#?}\n", &final_url, json);
        movie.name = json["Title"].to_string();
        movie.imdb = json["imdbID"].to_string();
        let year = json["Year"].to_string();
        movie.year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid

        println!("{:#?}", movie);
    }

    return Ok(results); // kind of shitty to double-wrap the error here. Could be confusing that the outer result is `Ok` but the inner vector that gets returned is full of bad results

}

fn debug_print() -> Result<(), Box<dyn std::error::Error>> {
    // let env = ENV.lock()?;
    let db  = DB.lock()?;
    println!("{:#?}", db.shows);
    println!("{:#?}", db.movies);

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize(&ENV);
    initialize(&DB);

    let _ = load_env();
    let _ = construct_db();
    let _ = populate_shows();
    let raw_api_result = api_stuff();
    if raw_api_result.is_err() { return Err(err!("Had unrecoverable error in `api_stuff`")); }
    let api_results = raw_api_result.unwrap();

    if api_results.iter().count() != 0 {
        eprintln!("Had API Failures");
        for fail in api_results.iter() {
            eprintln!("{}", fail);
        }
    }

    Ok(())
}
