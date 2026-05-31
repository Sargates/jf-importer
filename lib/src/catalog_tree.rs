use std::path::PathBuf;
use std::ffi::OsString;
use std::env;
use std::sync::{Arc, Weak};
use std::cell::RefCell;

use tokio::sync::Mutex;
use futures::executor::block_on;

use crate::media_item::{Movie, Show, Episode};
// use crate::api_query::QueryResponse;
use crate::api::QueryResponse;
use crate::dir_search::CreateError;

pub struct CatalogTree {
    pub movies: Vec<Arc<Mutex<Movie>>>,
    pub shows: Vec<Arc<Mutex<Show>>>,
    pub tree: TreeNode
}

#[derive(Debug)]
pub enum TreeGenError {
    EnvVarNotSet,
    EnvVarNotUnicode(OsString),
    MoviesDirDoesntExist,
    ShowsDirDoesntExist,
    RegexSyntaxError(String),
    RegexTooBig(usize),
    RegexUnknown,
}
impl From<std::env::VarError> for TreeGenError {
    fn from(value: std::env::VarError) -> Self {
        match value {
            env::VarError::NotPresent => Self::EnvVarNotSet,
            env::VarError::NotUnicode(os_string) => Self::EnvVarNotUnicode(os_string),
        }
    }
}
impl From<regex::Error> for TreeGenError {
    fn from(value: regex::Error) -> Self {
        match value {
            regex::Error::Syntax(syntax) => Self::RegexSyntaxError(syntax),
            regex::Error::CompiledTooBig(size) => Self::RegexTooBig(size),
            _ => Self::RegexUnknown
        }
    }
}

#[derive(Debug)]
pub enum TreeNode {
    Category {
        name: String,
        children: Vec<TreeNode>
    },
    Movie(Arc<Mutex<Movie>>),
    Show {
        show: Arc<Mutex<Show>>,
        children: Vec<TreeNode>
    },
    Episode(Arc<Mutex<Episode>>),
    Fail {
        err: CreateError,
        buf: PathBuf
    },
}
impl TreeNode {
    pub fn children(&self) -> &Vec<TreeNode> {
        match self {
            TreeNode::Category{ name, children } => { children }
            TreeNode::Show{ show, children } => { children }
            _ => panic!("Expected variant with children")
        }
    }
    pub fn children_mut(&mut self) -> &mut Vec<TreeNode> {
        match self {
            TreeNode::Category{ name, children } => { children }
            TreeNode::Show{ show, children } => { children }
            _ => panic!("Expected variant with children")
        }
    }
    pub fn push_child(&mut self, child: TreeNode) {
        match self {
            TreeNode::Category{ name, children } => { children.push(child); }
            TreeNode::Show{ show, children }     => { children.push(child); }
            _                                    => panic!("Expected variant with children")
        }
    }
    pub fn is_queried(&self) -> bool {
        match self {
            TreeNode::Show{ show, .. } => block_on(show.lock()).query    != None,
            TreeNode::Movie(movie)     => block_on(movie.lock()).query   != None,
            TreeNode::Episode(episode) => block_on(episode.lock()).query != None,
            _                          => panic!("Expected Movie, Show, or Episode!")
        }
    }
}
impl std::fmt::Display for TreeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // writeln!(f, "Printing: {:?}", self);
        match self {
            TreeNode::Category{ name, children } => {
                write!(f, "{name}")
            }
            TreeNode::Show{ show, children } => {
                match show.try_lock() {
                    Ok(guard) => {
                        match &guard.query {
                            Some(response) => {
                                write!(f, "{} ({}) ", response.title, response.year);
                                if let Some(imdb) = &response.imdb {
                                       write!(f, "[imdbid-{}]", imdb) } 
                                else { write!(f, "[tmdbid-{}]", response.tmdb) }
                            }
                            None => write!(f, "{}", guard.src.to_string_lossy()),
                        }
                    }
                    Err(err) => write!(f, "Failed to get lock for Episode! Error: {:?}", err)
                }
            }
            TreeNode::Movie(movie) => {
                match movie.try_lock() {
                    Ok(guard) => {
                        match &guard.query {
                            Some(response) => {
                                write!(f, "{} ({}) ", response.title, response.year);
                                if let Some(imdb) = &response.imdb {
                                    write!(f, "[imdbid-{}]", imdb) } 
                                else { write!(f, "[tmdbid-{}]", response.tmdb) }
                            }
                            None => write!(f, "{}", guard.src.to_string_lossy()),
                        }
                    }
                    Err(err) => write!(f, "Failed to get lock for Episode! Error: {:?}", err)
                }
            }
            TreeNode::Episode(ep) => {
                // I used to get deadlock if I used `block_on(ep.lock())`, but doing it this way is better 
                // because there shouldn't be any risk of deadlocking in synchronous code.
                match ep.try_lock() {
                    Ok(guard) => {
                        match (&guard.query, Weak::upgrade(&guard.parent).unwrap().try_lock()) {
                            (_, Ok(parent_guard)) 
                                if parent_guard.query.is_some() => write!(f, "{} {}", parent_guard.query.as_ref().unwrap().title.clone(), guard.id),
                            (_, Ok(parent_guard)) 
                                if parent_guard.query.is_none() => write!(f, "Parent query is empty: {}", guard.src.to_string_lossy()),
                            (_, Err(err)        )               => write!(f, "Failed to get Parent Lock: {}", guard.src.to_string_lossy()),
                            _                                   => unreachable!()
                        }
                    }
                    Err(err) => write!(f, "Failed to get lock for Episode! Error: {:?}", err)

                }
            }
            TreeNode::Fail{ err, buf } => {
                write!(f, "Parse Failure: {buf:?}")
            }
        }
    }
}
