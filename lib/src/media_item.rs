use std::fmt;
use std::path::Path;
use crate::api_query::*;

trait ApiQuery {

}

//# I don't want to have to deal with invalid Movie or Episode objects
// TODO: Guarantee a Movie/Episode is valid before creating one
// fn new(path: &str) -> Result(Movie, Self::Error);
// Check that `new` succeeded and skip if it didn't

#[derive(Debug)]
pub struct Movie {
    pub src: String,
    pub query: QueryResponse
}
impl Movie {
    pub(crate) fn new(src: String) -> Self {
        Movie {
            src,
            query: QueryResponse::None
        }
    }
}

#[derive(Debug)]
pub struct Show {
    pub src: String, // directory containing show
    pub query: QueryResponse
}
impl Show {
    pub(crate) fn new(src: String) -> Self {
        Show {
            src,
            query: QueryResponse::None
        }
    }
}

#[derive(Debug)]
pub struct Episode {
    pub src: String,
    pub query: QueryResponse,
}
impl Episode {
    pub(crate) fn new(src: String) -> Self {
        Episode {
            src,
            query: QueryResponse::None
        }
    }
}

pub trait Movable {
    type Error;
    /// `move` wave taken :/
    fn relocate(&self) -> Result<(),Self::Error>;
}

#[derive(Debug, Clone)]
pub enum MappingError {
    FailedToMapPath
}

pub trait Mappable {
    type Error;
    fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<String,Self::Error>;
}

impl Mappable for Movie {
    type Error = MappingError;
    fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<String,Self::Error> {
        todo!()
    }
}

impl Mappable for Episode {
    type Error = MappingError;
    fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<String,Self::Error> {
        todo!()
    }
}
