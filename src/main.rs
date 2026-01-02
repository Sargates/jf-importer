#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused)]

//# TUI Program
//  Interactive TUI program like ranger
//     Vim controls: `hjkl`, other motions
//     Display detected shows and movies
//        Ability to select individual entries and manually begin importing
//*       Rust library for moving a lot of files at a time? Maybe something where you can create a manifest and begin manually
//?    Do I need an event loop to do things?
//        A terminal isn't like OpenGL/Vulkan. You don't need to make draw calls (updates) constantly to tell Windows a program hasn't crashed
//        Asynchronous File moving?? Check Moving periodically?
//?         Is there a way to get a progress bar on moving a file? Probably not
//     Ability to add an override for a video manually manually
//       API Overview, display what the API returns when

//#  Process:
//   Parse dotenv
//     Make sure that src and dst directories exist already
//?  Check existing files
//?    Enumerate `dst_dir`
//   Enumerate `src_dir`
// ! Perform Checks
//     That show is in correct format
//     Don't overwrite episodes by default
//   Move files

// TODO: 
// Schema
//    How to represent TV shows?
//    Map of strings to paths as strings? 
//
//    How to represent Movies?
//    Path and info

mod types;
mod query;
mod process;


use types::*;
use process::*;

use reqwest;
use regex::Regex;
use std::error::Error;
use std::path::Path;

fn api_stuff(app: &mut App) -> Result<Vec<Box<dyn Error>>, Box<dyn std::error::Error>> {
    let responses_shows  = Api::fetch_api_shows(&app)?;
    let responses_movies = Api::fetch_api_movies(&app)?;
    let year_re = Regex::new(r"^[0-9]+")?;

    let mut results: Vec<Box<dyn Error>> = vec![];
    
    if responses_shows.iter().count() == 0 {
        results.push(err!("Got no responses for shows from TMDB API"));
    }
    for (i, response) in responses_shows {
        let opt = app.db.shows.get(i);
        if opt.is_none() { results.push(err!("No show object at element {}", i));
                           continue; }
        let mut show = opt.unwrap().borrow_mut();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["total_results"].as_u64().unwrap_or(0) == 0 {
            results.push(err!("API Failure for show: {:#?}", Path::new(&show.root_dir).file_name().unwrap()));
            continue; }

        let json = &raw_json["results"][0];
        show.title = json["name"].to_string();
        show.tmdb = json["id"].to_string();
        let year = json["first_air_date"].to_string();
        show.start_year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid

        // println!("{:#?}", show);

    }
    if responses_movies.iter().count() == 0 {
        results.push(err!("Got no responses for movies from OMDB API"));
    }

    for (i, response) in responses_movies {
        let opt = app.db.movies.get_mut(i);
        if opt.is_none() { results.push(err!("No show object at element {}", i));
                           continue; }
        let movie = opt.unwrap();

        let search_term = Path::new(&movie.src).file_stem().unwrap().to_str().unwrap().to_string();
        let hardcoded: bool;
        if let Ok(res) = Api::check_override(&search_term) { hardcoded = true; }
        else                                          { hardcoded = false; }

        let final_url = response.url().to_string().clone();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["Response"] == "False" { results.push(err!("API Failure for movie: {:#?}", &search_term));
                                             continue; }

        let json;
        if hardcoded { json = &raw_json;              } // response changes if we search directly w/ an imdb id or a string
        else         { json = &raw_json["Search"][0]; }

        movie.title = json["Title"].to_string();
        movie.imdb = json["imdbID"].to_string();
        let year = json["Year"].to_string();
        movie.year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid

        // println!("{:#?}", movie);
    }

    return Ok(results); //* kind of shitty to double-wrap the error here. Could be confusing that the outer result is `Ok` but the inner vector that gets returned is full of bad results

}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()
        .load_env()?         // Load from `.env` and `base.env`
        .build_db()?         // Build database
        .enumerate_shows()?; // Enumerate shows
    
    let _ = api_stuff(&mut app);

    let dst_dir = app.env.dst_dir.clone() + "/" + &app.env.show_dir_name;
    for show in app.db.shows.iter() {
        let reference = show.borrow();
        for episode in reference.episodes.iter() {
            if let Ok(path) = VideoFile::mapped_path(episode, &dst_dir) { 
                   println!("{} -> {}", episode.src, path); } 
            else { println!("{} -> {}", episode.src, "FAILURE!!!"); }
        }
    }
    for movie in app.db.movies.iter() {
        if let Ok(path) = VideoFile::mapped_path(movie, &dst_dir) { 
               println!("{} -> {}", movie.src, path); } 
        else { println!("{} -> {}", movie.src, "FAILURE!!!"); }
    }


    Ok(())
}
