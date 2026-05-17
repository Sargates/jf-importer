#![allow(dead_code)]
#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused)]

use std::error::Error;
use std::path::Path;
use std::mem::drop;

use jf_importer;


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
