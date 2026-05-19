#![allow(nonstandard_style)]
#![allow(unused_variables)]
#![allow(unused)]

use core::slice;
use std::error::Error;
use std::io::LineWriter;
use std::path::Path;
use std::mem::drop;
use std::alloc;

use jf_importer::api_query::{QueryError, Queryable};
use jf_importer::{catalog_tree, dir_search, env};


//* IMPORTS USED FOR TESTING. THESE SHOULD BE GONE WHEN FINISHED
use reqwest::blocking::{
    Client,
    RequestBuilder,
    Response,
};

fn recursive_print(node: &catalog_tree::TreeNode, old_indent: String) {
    // `tree` ripoff
    const connector: &'static str = "│   ";
    const middle:    &'static str = "├── ";
    const end:       &'static str = "└── ";
    const empty:     &'static str = "    ";

    print!("{}", old_indent);
    println!("{node}");

    let mut next_indent = String::new();
    if old_indent.chars().count() > 3 {
        let split_point = old_indent.char_indices().rev().nth(3).map_or(0, |(idx, ch)| idx);
        let (rest, last) = old_indent.split_at(split_point);
        
        match &last.chars().nth(0).unwrap() {
            '├' => { next_indent = String::from(rest) + connector }
            '└' => { next_indent = String::from(rest) + empty }
             _  => {}
        }
    }

    match &node {
        catalog_tree::TreeNode::Show {show, children} => {
            for child in children {
                let mut copy = next_indent.clone();
                let last = children.last().unwrap();
                // ref: https://users.rust-lang.org/t/is-any-way-to-know-references-are-referencing-the-same-object/9716/6
                if child as *const _ != children.last().unwrap() as *const _ 
                     { copy += middle; }
                else { copy += end; }
                recursive_print(child, copy.clone());
            }
        }
        catalog_tree::TreeNode::Category { name, children } => {
            if name == "Failures" { return; }
            for child in children {
                let mut copy = next_indent.clone();
                let last = children.last().unwrap();
                // ref: https://users.rust-lang.org/t/is-any-way-to-know-references-are-referencing-the-same-object/9716/6
                if child as *const _ != children.last().unwrap() as *const _ 
                     { copy += middle; }
                else { copy += end; }
                recursive_print(child, copy.clone());
            }
        }
        _ => {}
    }
}

fn main() -> () {
    if let Err(err) = env::load_env() {
        println!("Failed to load env: {:?}", err);
        return;
    }

    let res = dir_search::generate_catalog_tree();

    if let Err(err) = &res {
        println!("Failed: {:?}", err);
        return;
    }
    let mut tree = res.unwrap();

    println!("Success!");

    println!();
    println!();
    let client = Client::new();

    println!("{}", std::env::var("VIDEO_FILE_EXTENTIONS").unwrap());

    let mut categories = tree.tree.children_mut();

    let movies = categories.get_mut(0).unwrap().children_mut();
    for movie in movies {
        if let catalog_tree::TreeNode::Movie(movie) = movie {
            let res = movie.borrow_mut().query_api(&client);
            if let Err(err) = res {
                println!("Query Failure: {err:?} for {}", movie.borrow().src.to_string_lossy());
                return;
            }
            let response = res.unwrap();
            let status = movie.borrow_mut().process_response(response);
            if let Err(err) = status {
                println!("JSON Processing Failure: {err:?} for {}", movie.borrow().src.to_string_lossy());
                return;
            }
        }
    }

    let shows = categories.get_mut(1).unwrap().children_mut();
    for show_node in shows {
        print!("{}: ", show_node);
        println!("{}", show_node.children_mut().len());
        if let catalog_tree::TreeNode::Show{ show, children } = show_node {
            let res = show.borrow_mut().query_api(&client);
            if let Err(err) = res {
                println!("Query Failure: {err:?} for {}", show.borrow().src.to_string_lossy());
                continue;
            }
            let response = res.unwrap();
            let status = show.borrow_mut().process_response(response);
            if let Err(err) = status {
                match err {
                    QueryError::FailedToExtractApiData(json) => {
                        println!("JSON Processing Failure: {json}");
                    }
                    _ => {
                        println!("Failed to process API response: {err:?}");
                    }
                }
                continue;
            }
            // println!("{}: {}", show_node, children.len());
            for child_node in children {
                if let catalog_tree::TreeNode::Episode(episode) = child_node {
                    // This is so fucking dumb but I don't want to refactor `Queryable`
                    let res = reqwest::blocking::get("http://127.0.0.1:13000");
                    if let Err(err) = res { panic!("Create a dummy webserver on port 13000!"); }
                    let dummy_response = res.unwrap();

                    let status = episode.borrow_mut().process_response(dummy_response);
                    if let Err(err) = status {
                        match err {
                            _ => {
                                println!("Failed to process Episode API response: {err:?}");
                            }
                        }
                        continue;
                    }
                }
            }
        }
    }

    recursive_print(&tree.tree, String::new());
}
