use std::{ fmt, fs, io };
use ignore::*;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use regex::Regex;
use std::path::{ Path, PathBuf };
use std::rc::Rc;
use std::cell::RefCell;

use crate::api_query::{ Queryable, QueryResponse };
use crate::media_item::{ Movie, Show, Episode, Mappable };
use crate::catalog_tree::{CatalogTree, TreeGenError, TreeNode};


//? What if we just expose library functions to query the ENV-defined movies and shows directories and returns owned vectors of their respective types?
//? Or a `generate_catalogue_tree()` function? 


#[derive(Debug)]
pub enum CreateError {
    FailedToCreateMediaItem,
    PathNotUnicode,
    IncorrectFileTypeSupplied,
    EpisodeIncorrectFormat,
    EpisodeFailedToParseSeason,
}

pub fn generate_catalog_tree() -> Result<CatalogTree, TreeGenError> {
    let movies_dir = env::var("SRC_DIR")? + "/movies"; // TODO: don't hardcode these subdirectories
    let shows_dir  = env::var("SRC_DIR")? + "/shows";  // TODO: don't hardcode these subdirectories

    if ! Path::new(&movies_dir).exists()
        { return Err(TreeGenError::MoviesDirDoesntExist); }
    if ! Path::new(&shows_dir).exists()
        { return Err(TreeGenError::ShowsDirDoesntExist); }

    // I HAVE NEVER GOTTEN TO USE CURRYING BEFORE!!! LET'S GO
    let currying_match = |re: Regex, invert: bool| 
        move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) };

    let ignore_re = Regex::new(r".*(ignore|temp).*")?;
    let video_re = Regex::new(&(env::var("VIDEO_FILE_EXTENTIONS")?))?;
    
    let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
    let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
    let is_match = move |x: &String, re: &Regex| re.is_match(x);

    
    let mut root = TreeNode::Category{ name: String::from("."), children: vec![] };
    let mut movies_cat: TreeNode = TreeNode::Category { name: String::from("Movies"), children: Vec::new()};
    let mut shows_cat:  TreeNode = TreeNode::Category { name: String::from("Shows"),  children: Vec::new()};
    let mut fails_cat:  TreeNode = TreeNode::Category { name: String::from("Failures"),  children: Vec::new()};

    let mut movies = vec![];
    let mut shows = vec![];

    let mut builder = WalkBuilder::new(movies_dir);
    builder.min_depth(Some(1))
        .max_depth(Some(1))
        .filter_entry(currying_match(ignore_re.clone(), true))
        .filter_entry(currying_match(video_re.clone(), false));
    for result in builder.build() {
        if let Err(err) = result {
            // ! Use `tracing` for logging
            //* Going to be weird. 
            //* Needs a setup call, but we can't do that in the library because then that locks the end user into using exactly that.
            //* Also can't not call the setup function in the library because then we're calling random log functions in the library for no reason.
            //* IDK
            println!("Bad Dir Entry {}", err);
            continue;
        }
        let dir_entry = result.unwrap();
        if ! dir_entry.path().is_file() { continue; }

        // let movies = movies_cat.children_mut();
        let create_result = Movie::new(dir_entry.path().to_path_buf());
        if let Err(err) = create_result {
            let fails = fails_cat.children_mut();
            let buf = dir_entry.path().to_path_buf();
            fails.push(TreeNode::Fail{err, buf});
            continue;
        }
        let movie = create_result.unwrap();
        movies.push(Rc::new(RefCell::new(movie)));
    }
    drop(builder);


    let mut builder = WalkBuilder::new(shows_dir);
    builder.min_depth(Some(1))
        .max_depth(Some(1))
        .filter_entry(currying_match(ignore_re.clone(), true));
    for result in builder.build() {
        if let Err(err) = result {
            // ! Use `tracing` for logging
            //* Going to be weird. 
            //* Needs a setup call, but we can't do that in the library because then that locks the end user into using exactly that.
            //* Also can't not call the setup function in the library because then we're calling random log functions in the library for no reason.
            //* IDK
            println!("Bad Dir Entry {}", err);
            continue;
        }
        let dir_entry = result.unwrap();
        if ! dir_entry.path().is_dir() { continue; }

        // let shows = shows_cat.children_mut();
        let create_result = Show::new(dir_entry.path().to_path_buf());
        if let Err(err) = create_result {
            let fails = fails_cat.children_mut();
            let buf = dir_entry.path().to_path_buf();
            fails.push(TreeNode::Fail{err, buf});
            continue;
        }
        let show = create_result.unwrap();
        shows.push(Rc::new(RefCell::new(show)));
    }
    drop(builder);

    for show in shows.iter_mut() {
        println!("Processing Show: {}", &show.borrow().src.to_string_lossy());
        let mut builder = WalkBuilder::new(&show.borrow().src);
        builder.min_depth(Some(1))
            .sort_by_file_path(|a, b| a.cmp(b)); // a < b
        for result in builder.build() {
            if let Err(err) = result {
                // ! Use `tracing` for logging
                //* Going to be weird. 
                //* Needs a setup call, but we can't do that in the library because then that locks the end user into using exactly that.
                //* Also can't not call the setup function in the library because then we're calling random log functions in the library for no reason.
                //* IDK
                println!("Bad Dir Entry {}", err);
                continue;
            }
            let dir_entry = result.unwrap();
            if ! dir_entry.path().is_file() { continue; } // skip directories

            let create_result = Episode::new(dir_entry.path().to_path_buf());
            if let Err(err) = create_result {
                let fails = fails_cat.children_mut();
                let buf = dir_entry.path().to_path_buf();
                fails.push(TreeNode::Fail{err, buf});
                continue;
            }
            let episode = create_result.unwrap();
            show.borrow_mut().episodes.push(Rc::new(RefCell::new(episode)));
        }
    }

    for movie_box in movies.iter() {
        movies_cat.push_child(TreeNode::Movie(movie_box.clone()));
    }

    for show_box in shows.iter() {
        let show = show_box.clone();
        let mut children = vec![];
        for episode in show.borrow().episodes.iter() {
            children.push(TreeNode::Episode(episode.clone()));
        }
        shows_cat.push_child(TreeNode::Show { show, children });
    }

    let children = root.children_mut();
    children.push(movies_cat);
    children.push(shows_cat);
    children.push(fails_cat);

    Ok(CatalogTree {
        movies,
        shows,
        tree: root
    })
}
