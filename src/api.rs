use reqwest::blocking::{Response, Request};
use std::fs;
use regex::Regex;
use urlencoding;
use std::path::Path;


use crate::types;
use types::*;

pub fn query_api(builder: reqwest::blocking::RequestBuilder, url: String) -> Result<Response, reqwest::Error> { builder.send() }
pub fn query_for_movie(movie_in: &String, client: &reqwest::blocking::Client, env: &env_t) -> Result<Response, reqwest::Error> {
    let searched_query: String;

    // Need to differentiate between searching with a raw string or by its imdb_id
    let imdb_id_re = Regex::new(r"^tt[0-9]+").unwrap(); // TODO when the fuck does this fail?


    if let Ok(res) = check_override(movie_in) { searched_query = res; } // set movie if hardcoded, otherwise just copy movie_in
    else                                      { searched_query = movie_in.clone(); } 

    let url;
    if imdb_id_re.is_match(&searched_query) { url = format!("https://www.omdbapi.com/?apikey={}&type=movie&i={}", env.omdb_key, searched_query); }
    else                                    { url = format!("https://www.omdbapi.com/?apikey={}&type=movie&s={}*", env.omdb_key, urlencoding::encode(&searched_query)); }
    let builder = client.clone().get(&url);
    query_api(builder, url.clone())
}

pub fn check_override(movie: &String) -> Result<String, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string("manual-fixes.json")?;
    let json = json::parse(&contents)?;
    // eprintln!("{} {} {}", json, movie, json[movie]);
    if json[movie].is_null() { return Err(err!("Movie is not hardcoded")); }
    let id = json[movie].to_string();
    Ok(id)
}

pub fn query_for_show(show: &String, client: &reqwest::blocking::Client, env: &env_t) -> Result<Response, reqwest::Error> {
    let encoded = urlencoding::encode(&show);
    let url = format!("https://api.themoviedb.org/3/search/tv?query={}", encoded);
    let builder = client.clone().get(&url)
        .header("Authorization", format!("Bearer {}", env.tmdb_key))
        .header("accept", "application/json");

    query_api(builder, url.clone())
}

pub fn fetch_api_shows(env: &env_t, db: &db_t) -> Result<Vec<(usize, Response)>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();

    let mut results: Vec<(usize, Response)> = vec![];

    for (i, show) in db.shows.iter().enumerate() {
        let search_query = Path::new(&show.root_dir).file_name().unwrap().to_str().unwrap().to_string();
        let result = query_for_show(&search_query, &client, &env)?;
        results.push((i, result));
    }

    Ok(results)
}

pub fn fetch_api_movies(env: &env_t, db_lock: &db_t) -> Result<Vec<(usize, Response)>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();

    let mut results: Vec<(usize, Response)> = vec![];

    for (i, movie) in db_lock.movies.iter().enumerate() {
        let search_query = Path::new(&movie.path).file_stem().unwrap().to_str().unwrap().to_string();
        let result = query_for_movie(&search_query, &client, &env)?;
        results.push((i, result));
    }

    Ok(results)
}

