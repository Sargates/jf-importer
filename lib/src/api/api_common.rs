use std::sync::Arc;
use std::error::Error;
use tokio::sync::Mutex;

use async_trait::async_trait;

use crate::media::{Movie, Show};
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

/// Abstracted out response object. Only the things we care about (for now)
// TODO: make these `pub(crate)`
#[derive(Debug, Hash)]
pub struct QueryResponse {
    pub title: String,
    pub year: u32,
    pub imdb: Option<String>,

    /// (Currently) The TMDB of the item that owns the response.
    // TODO: Make this `enum ApiSpecificId { TMDB(String), OMDB(String) }`
    pub tmdb: String, 
}

// GLORIOUS CRATE!!!
#[async_trait]
pub trait ApiClient {
    async fn search_movie(&self, movie: Arc<Movie>) -> QueryStatus;
    async fn search_show(&self, show: Arc<Show>) -> QueryStatus;
}
