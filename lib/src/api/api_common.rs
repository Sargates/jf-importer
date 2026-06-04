use std::sync::Arc;
use tokio::sync::Mutex;

use async_trait::async_trait;

use crate::media_item::{Movie, Show};

/// The status of an outgoing query to an API
#[derive(Debug)]
pub enum QueryStatus {
    Failed(QueryError),
    NotStarted,
    InProgress,
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

/// Errors that can occur in the act of querying an API
#[derive(Debug)]
pub enum QueryError {
    /// For TreeNode variants that aren't Movie, Show, Episode
    CantQueryOnType, 

    /// Api key is empty when attempting to query media.
    /// Likely that SECRETS reverted to `Default::default()`
    UnsetApiKey,

    /// Mappings from `reqwest::Error`
    ReqwestError(reqwest::Error),

    // Mappings from JsonParseError
    SerdeDeserializeError(serde_json::Error),

    TMDBIncompleteResponse,
    TMDBFailedToConvertFromResponse,

    /// HTTP 429, see https://developer.themoviedb.org/docs/rate-limiting
    /// There isn't actually any respecting of this quite yet.
    // TODO: Respect this response
    TMDBTooManyRequests, 

    InvalidApiKey,
    FailedToQueryApi,
    NoSearchResultsFromApi,
    FailedToExtractApiData(String),
    // TODO: Reference actual API responses
    //? Create unified enum for different APIs?
}
impl From<reqwest::Error> for QueryError {
    fn from(value: reqwest::Error) -> Self {
        if value.status().is_some() && 
            value.status().unwrap() == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            // TODO: Don't make this TMDB-specific
            return QueryError::TMDBTooManyRequests; 
        }
        QueryError::ReqwestError(value)
    }
}
impl From<serde_json::Error> for QueryError {
    fn from(value: serde_json::Error) -> Self {
        QueryError::SerdeDeserializeError(value)
    }
}
// Apparently it's not common to implement `PartialEq` on error types
// WHY????
// https://github.com/seanmonstar/reqwest/issues/471#issuecomment-471114308
// https://github.com/bincode-org/bincode/pull/273#issuecomment-513773714
impl PartialEq for QueryError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FailedToExtractApiData(l0), Self::FailedToExtractApiData(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
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
    // fn semaphore_req() -> impl std::future::Future<Output = >
    async fn search_movie(&self, movie: Arc<Movie>) -> QueryStatus;
    async fn search_show(&self, show: Arc<Show>) -> QueryStatus;
}
