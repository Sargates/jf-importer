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
use types::*;

mod query;

mod process;
use process::*;
// mod error; use error::*;

// mod ui;
// use ui::*;

use reqwest;
use regex::Regex;

use std::error::Error;
use std::path::Path;
use std::mem::drop;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()
        .load_env()?         // Load from `.env` and `base.env`
        .build_db()?         // Build database
        .enumerate_shows()?; // Enumerate shows

    let dst_dir = app.env.dst_dir.clone() + "/" + &app.env.show_dir_name;
    for show in app.db.shows.iter_mut() {
        let mut reference = show.borrow_mut();

        let response = reference.make_api_call(&app.request_client, &app.env.tmdb_key)?;
        if let Err(err) = reference.update_info(response) { //* `response` is not changed if we errored out
            eprintln!("API FAILURE: {}", reference.working_title);
        }
    }

    for show in app.db.shows.iter() {
        let reference = show.borrow();

        for episode in reference.episodes.iter() {
            if let Ok(path) = episode.mapped_path(&dst_dir) {
                   println!("{} -> {}", episode.src, path); } 
            else { println!("{} -> {}", episode.src, "FAILURE!!!"); }
        }
    }

    for movie in app.db.movies.iter_mut() {
        let response = movie.make_api_call(&app.request_client, &app.env.omdb_key)?;
        if let Err(err) = movie.update_info(response) { //* `movie` is not changed if we errored out
            eprintln!("API FAILURE: {}", movie.working_title);
        }

        if let Ok(path) = movie.mapped_path(&dst_dir) {
               println!("{} -> {}", movie.src, path); } 
        else { println!("{} -> {}", movie.src, "FAILURE!!!"); }
    }


    Ok(())
}
