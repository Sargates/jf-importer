use std::sync::Arc;
use std::error::Error;
use tokio::sync::Mutex;

use async_trait::async_trait;

use crate::catalog::{Movie, Show};
use super::error::QueryError;

/// The status of an outgoing query to an API
#[derive(Debug)]
pub enum QueryStatus {
    NotStarted,
    InProgress,
    Failed(QueryError),
    Success(QueryResponse),
}
impl QueryStatus {
    pub fn to_string(&self, fallback: String) -> String {
        match self {
            QueryStatus::Success(query) => {
                query.title.clone()
            }
            QueryStatus::Failed(err) => {
                String::from(format!("[Failed] {}", fallback))
            }
            QueryStatus::NotStarted => {
                String::from(format!("[NotStarted] {}", fallback))
            }
            QueryStatus::InProgress => {
                String::from(format!("[InProgress] {}", fallback))
            }
        }
    }
}

#[derive(Hash, Debug)]
pub enum ApiSpecificMediaId {
    // IMDB doesn't actually have an API (that I'm aware of).
    // So this is acquired from OMDB or by indirection with TMDB
    /// form: `imdbid-tt[0-9]+`
    // IMDB(String),

    /// form: `tmdbid-[0-9]+`
    TMDB(String),

    // Uses IMDB ids
    /// form: `imdbid-tt[0-9]+`
    OMDB(String)
}
/// Abstracted out response object. Only the things we care about (for now)
// TODO: make these `pub(crate)`
#[derive(Debug, Hash)]
pub struct QueryResponse {
    pub title: String,
    pub year: u32,
    pub imdb: Option<String>,

    /// (Currently) The TMDB of the item that owns the response.
    // TODO: Make this `enum ApiSpecificId { TMDB(String), OMDB(String) }`
    pub tmdb: ApiSpecificMediaId, 
}

// GLORIOUS CRATE!!!
#[async_trait]
pub trait ApiClient {
    async fn search_movie(&self, movie: Arc<Movie>) -> QueryStatus;
    async fn search_show(&self, show: Arc<Show>) -> QueryStatus;
}
