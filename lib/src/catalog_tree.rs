use std::path::PathBuf;
use std::ffi::OsString;
use std::env;
use std::rc::Rc;
use std::cell::RefCell;

use crate::media_item::{Movie, Show, Episode};
use crate::api_query::QueryResponse;
use crate::dir_search::CreateError;

pub struct CatalogTree {
    pub movies: Vec<Rc<RefCell<Movie>>>,
    pub shows: Vec<Rc<RefCell<Show>>>,
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
    Movie(Rc<RefCell<Movie>>),
    Show {
        show: Rc<RefCell<Show>>,
        children: Vec<TreeNode>
    },
    Episode(Rc<RefCell<Episode>>),
    Fail {
        err: CreateError,
        buf: PathBuf
    },
}
impl TreeNode {
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
            TreeNode::Show{ show, .. } => show.borrow().query    != QueryResponse::None,
            TreeNode::Movie(movie)     => movie.borrow().query   != QueryResponse::None,
            TreeNode::Episode(episode) => episode.borrow().query != QueryResponse::None,
            _                          => panic!("Expected Movie, Show, or Episode!")
        }
    }
}
impl std::fmt::Display for TreeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreeNode::Category{ name, children } => {
                write!(f, "{name}")
            }
            TreeNode::Show{ show, children } => {
                match &show.borrow().query {
                    QueryResponse::Show{title, start_year, tmdb} => write!(f, "{title} ({start_year}) [tmdbid-{tmdb}]"),
                    QueryResponse::None                          => write!(f, "{}", show.borrow().src.to_string_lossy()),
                    _                                            => panic!("Show has invalid query response!")
                }
            }
            TreeNode::Movie(movie) => {
                match &movie.borrow().query {
                    QueryResponse::Movie{title, year, imdb} => write!(f, "{title} ({year}) [imdbid-{imdb}]"),
                    QueryResponse::None                     => write!(f, "{}", movie.borrow().src.to_string_lossy()),
                    _                                       => panic!("Show has invalid query response!")
                }
            }
            TreeNode::Episode(ep) => {
                let borrow = ep.borrow();
                match &borrow.query {
                    QueryResponse::Episode{show_title} => write!(f, "{} {}", show_title, borrow.id),
                    QueryResponse::None                => write!(f, "{}", ep.borrow().src.to_string_lossy()),
                    _                                  => panic!("Show has invalid query response!")
                }
            }
            TreeNode::Fail{ err, buf } => {
                write!(f, "Parse Failure: {buf:?}")
            }
        }
    }
}
