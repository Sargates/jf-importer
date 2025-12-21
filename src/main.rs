#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![feature(default_field_values)]

use core::net;
use std::fmt;
use std::fmt::Display;
use std::pin::Pin;

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
use urlencoding::encode;

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

impl fmt::Display for BasicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Show the inner message
        write!(f, "{}", self.0)
    }
}

impl Error for BasicError {}

/*
// To sort from shell script

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

// #[derive(Debug)]
pub struct Movie {
    pub name: String = String::new(),
    pub path: String = String::new(),
    pub year: String = String::new(),
    pub imdb: String = String::new()
}

impl fmt::Debug for Movie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({}) [{}] - ({})", self.name, self.imdb, self.year, self.path)
    }
}

// #[derive(Debug)]
pub struct Show {
    pub name: String = String::new(),
    pub path: String = String::new(),
    pub year: String = String::new(),
    pub tmdb: String = String::new(),
    pub episodes: BTreeMap<String, String> = BTreeMap::new(), // "SXXEXX" -> "/path/to/episode"
}

impl fmt::Debug for Show {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{} ({}) [{}]", self.name, self.year, self.tmdb)?;
        for (episode, path) in self.episodes.iter() { writeln!(f, "{} -> {}", episode, path)?; }
        Ok(())
    }
}

pub struct env_t {
    pub src_dir:   String = String::new(),
    pub dst_dir:   String = String::new(),
    pub omdb_key:  String = String::new(),
    pub tmdb_key:  String = String::new(),
    pub cache_dir: String = String::new()
}

lazy_static! {
    pub static ref ENV: Mutex<env_t> = Mutex::new(env_t{ .. });
    pub static ref DB: Mutex<db_t> = Mutex::new(db_t{ .. });
}

// static ENV: Mutex<env_t> = Mutex::new(env_t{ .. });

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

pub struct db_t {
    movies:  Vec<Movie> = Vec::new(),
    shows:   Vec<Show> = Vec::new()
}


fn construct_db() -> Result<(), Box<dyn Error>> {
    let env = ENV.lock()?;
    let mut db = DB.lock()?;
    let movies_path = env.src_dir.clone() + "/movies";
    let shows_path = env.src_dir.clone() + "/shows";
    let re = Regex::new(r".*(ignore|temp).*")?;
    let closure = move |x: &DirEntry| !re.is_match(x.file_name().to_str().unwrap());
    

    println!("{:?}", "Movies");
    for result in WalkBuilder::new(movies_path)
            .min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(closure.clone())
            .build() {
        let movie = result.unwrap();
        if ! movie.path().is_file() { continue; }
        db.movies.push(Movie {
            path: movie.path().to_str().unwrap().to_string(),
            ..
        });
        println!("    {:?}", movie.path().display());
    }
    println!("{:?}", "Shows");
    for result in WalkBuilder::new(shows_path)
            .min_depth(Some(1))
            .max_depth(Some(1))
            .filter_entry(closure.clone())
            .build() {
        let show = result.unwrap();
        if ! show.path().is_dir() { continue; }
        db.shows.push(Show {
            name: String::new(),
            path: show.path().to_str().unwrap().to_string(),
            tmdb: String::new(),
            year: String::new(),
            episodes: BTreeMap::new()
        });
        println!("    {:?}", show.path().display());
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
        for result in WalkBuilder::new(&show.path)
                .min_depth(Some(1))
                .sort_by_file_path(|a, b| a.to_str().unwrap().cmp(b.to_str().unwrap()))
                .build() {
            if is_dir(&result.clone()?) { continue; }
            let file = result?;
            let file_name = &file.path().to_str().unwrap().to_string();
            let path = &file.path().to_str().unwrap().to_string();

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

fn query_api(builder: reqwest::blocking::RequestBuilder, url: String) -> Result<Response, reqwest::Error> {
        builder.send()
}

// fn query_for_movie(movie: String, client: &reqwest::Client, env: &env_t) -> impl Future<Output = Result<Response, reqwest::Error>> {
//     // Need to differentiate between searching with a raw string or by its imdb_id
//     // See the legacy implementation
//     /* search_omdb_by_id() {
//         local imdb_id=$1
//             curl -s --request GET \
//             --url "https://www.omdbapi.com/?apikey=$OMDB_APIKEY&type=movie&i=$imdb_id"
//     }
//
//     search_omdb_by_name() {
//         local name=$1
//             # When Googling "Apple Pie", the space (" ") gets converted to an HTTP-readable format
//             # URI Encoding -> Universal Resource Identifier encoding
//             # URL: Apple%20Pie
//             local http_string="$(printf %s "$name" | jq -sRr @uri)"
//             curl -s --request GET \
//             --url "https://www.omdbapi.com/?apikey=$OMDB_APIKEY&type=movie&s=$http_string*" # use wildcard at end
//     } */
//
//     // let http_string = ""/* */;
//     // let url = format!("http://www.omdbapi.com/?apikey={}&type=movie&i={}", env.omdb_key, );
//     // let builder = client.clone().get(&url)
//     //     .header("Authorization", format!("Bearer {}", env.tmdb_key))
//     //     .header("accept", "application/json");
//     //
//     // query_api(builder, url.clone())
// }

// FOR POSTERITY -- The return value here is implicitly (not really but the reason why is esoteric) required to have `+ Unpin` when wrapped in a Box
// See https://stackoverflow.com/a/60562784
fn query_for_show(show: &String, client: &reqwest::blocking::Client, env: &env_t) -> Result<Response, reqwest::Error> {
    let encoded = encode(&show);
    let url = format!("https://api.themoviedb.org/3/search/tv?query={}", encoded);
    let builder = client.clone().get(&url)
        .header("Authorization", format!("Bearer {}", env.tmdb_key))
        .header("accept", "application/json");

    query_api(builder, url.clone())
}

fn get_api_responses(env_lock: &env_t, db_lock: &db_t) -> Result<Vec<(usize, Response)>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();

    let mut results: Vec<(usize, Response)> = vec![];

    for (i, show) in db_lock.shows.iter().enumerate() {
        let search_query = Path::new(&show.path).file_name().unwrap().to_str().unwrap().to_string();
        let result = query_for_show(&search_query, &client, &env_lock)?;
        results.push((i, result));
    }

    Ok(results)
}

fn api_stuff() -> Result<(), Box<dyn std::error::Error>> {
    let env = ENV.lock()?;
    let mut db  = DB.lock()?;
    let responses = get_api_responses(&env, &db)?;
    let year_re = Regex::new(r"^[0-9]+")?;
    
    let mut fail = 0;
    for (i, response) in responses {
        let opt = db.shows.get_mut(i);
        if opt.is_none() { fail = 2; continue; }
        let show = opt.unwrap();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["total_results"].as_u64().unwrap() == 0 { fail = 1; continue; }

        let json = &raw_json["results"][0];
        // println!("{}\n{:#?}\n", search_query, json);
        show.name = json["name"].to_string();
        show.tmdb = json["id"].to_string();
        let year = json["first_air_date"].to_string();
        show.year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid

        println!("{:#?}", show);

    }

    match fail {
        1 => Err(BasicError::boxed("API FAILURE".to_string())),
        2 => Err(BasicError::boxed(format!("No object at element"))),
        _ => Ok(()), // default
    }
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
    let _ = api_stuff();
    // let _ = debug_print();

    Ok(())
}
