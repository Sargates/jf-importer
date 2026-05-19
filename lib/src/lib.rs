// shut up rust analyzer!
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

pub mod env;
pub mod media_item;
pub mod api_query;
pub mod dir_search;
pub mod catalog_tree;

// use crate::env::*;
// use crate::media_item::*;
// use crate::api_query::*;
// use crate::dir_search::*;
//
// use dir_search::generate_catalog_tree;
