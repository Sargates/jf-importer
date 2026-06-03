use std::sync::Arc;
use tokio::sync::Mutex;

use async_trait::async_trait;

use crate::media_item::{Movie, Show};

#[derive(Debug, Clone)]
pub enum QueryError {
    /// For TreeNode variants that aren't Movie, Show, Episode
    CantQueryOnType, 

    /// Could be VarError::NotPresent or VarError::NotUnicode
    UnsetApiKey,

    /// Mappings from `reqwest::Error`
    ReqwestBuilder,
    ReqwestRedirect,
    ReqwestStatus,
    ReqwestTimeout,
    ReqwestRequest,
    ReqwestConnect,
    ReqwestBody,
    ReqwestDecode,
    ReqwestUpgrade,
    ReqwestUnknown,

    JsonParseError,

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
        if value.status().is_some() && value.status().unwrap() == reqwest::StatusCode::TOO_MANY_REQUESTS 
                                    { QueryError::TMDBTooManyRequests } // We respect this response
        else if value.is_builder()  { QueryError::ReqwestBuilder  }
        else if value.is_redirect() { QueryError::ReqwestRedirect } 
        else if value.is_status()   { QueryError::ReqwestStatus  } 
        else if value.is_timeout()  { QueryError::ReqwestTimeout  } 
        else if value.is_request()  { QueryError::ReqwestRequest  } 
        else if value.is_connect()  { QueryError::ReqwestConnect  } 
        else if value.is_body()     { QueryError::ReqwestBody     } 
        else if value.is_decode()   { QueryError::ReqwestDecode   } 
        else if value.is_upgrade()  { QueryError::ReqwestUpgrade } 
        else                        { QueryError::ReqwestUnknown }
    }
}
impl From<serde_json::Error> for QueryError {
    fn from(value: serde_json::Error) -> Self {
        QueryError::JsonParseError
    }
}

/// Abstracted out response object. Only the things we care about (for now)
// TODO: make these `pub(crate)`
#[derive(Debug, PartialEq, Hash)]
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
    async fn search_movie(&self, movie: Arc<Mutex<Movie>>) -> Result<(), QueryError>;
    async fn search_show(&self, show: Arc<Mutex<Show>>) -> Result<(), QueryError>;
}
