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
use std::error::Error;


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

pub mod types;
pub mod api;

pub use types::*;


fn api_stuff(app: &mut App) -> Result<Vec<Box<dyn Error>>, Box<dyn std::error::Error>> {
    let responses_shows  = api::fetch_api_shows(&app.env, &app.db)?;
    let responses_movies = api::fetch_api_movies(&app.env, &app.db)?;
    let year_re = Regex::new(r"^[0-9]+")?;

    let mut results: Vec<Box<dyn Error>> = vec![];
    
    if responses_shows.iter().count() == 0 {
        results.push(err!("Got no responses for shows from TMDB API"));
    }
    for (i, response) in responses_shows {
        let opt = app.db.shows.get_mut(i);
        if opt.is_none() { results.push(err!("No show object at element {}", i));
                           continue; }
        let show = opt.unwrap();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["total_results"].as_u64().unwrap_or(0) == 0 {
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
        let opt = app.db.movies.get_mut(i);
        if opt.is_none() { results.push(err!("No show object at element {}", i));
                           continue; }
        let movie = opt.unwrap();

        let search_term = Path::new(&movie.path).file_stem().unwrap().to_str().unwrap().to_string();
        let hardcoded: bool;
        if let Ok(res) = api::check_override(&search_term) { hardcoded = true; }
        else                                          { hardcoded = false; }

        let final_url = response.url().to_string().clone();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["Response"] == "False" { results.push(err!("API Failure for movie: {:#?}", &search_term));
                                             continue; }

        let json;
        if hardcoded { json = &raw_json;              } // response changes if we search directly w/ an imdb id or a string
        else         { json = &raw_json["Search"][0]; }

        movie.name = json["Title"].to_string();
        movie.imdb = json["imdbID"].to_string();
        let year = json["Year"].to_string();
        movie.year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid

        println!("{:#?}", movie);
    }

    return Ok(results); //* kind of shitty to double-wrap the error here. Could be confusing that the outer result is `Ok` but the inner vector that gets returned is full of bad results

}

fn debug_print(env: &mut env_t, db: &mut db_t) -> Result<(), Box<dyn std::error::Error>> {
    println!("{:#?}", db.shows);
    println!("{:#?}", db.movies);

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()
        .load_env()?
        .construct_db()?
        .populate_shows()?;


    let raw_api_result = api_stuff(&mut app);
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
