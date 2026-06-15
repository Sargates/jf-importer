/// Errors that can occur in the act of querying an API
#[derive(Debug)]
pub enum QueryError {
    /// For TreeNode variants that aren't Movie, Show, Episode
    CantQueryOnType, 

    /// Api key is empty when attempting to query media.
    /// Likely that SECRETS reverted to `Default::default()`
    UnsetApiKey,

    /// Response if the API call returned `null`
    InvalidApiKey,

    /// Generic Failure when quering an API
    // TODO: remove this
    FailedToQueryApi,

    /// From when API returns singular `null`
    NoSearchResultsFromApi,

    /// Mappings from `reqwest::Error`
    ReqwestError(reqwest::Error),

    // Mappings from serde_json parse error
    SerdeDeserializeError(serde_json::Error),

    /// Failed to fetch the IMDB ID of the media`
    TMDBIncompleteResponse,

    /// Failed to static convert type in response to another type
    /// Likely `str` -> `u32` conversion failure
    TMDBFailedToConvertFromResponse,

    /// HTTP 429, see https://developer.themoviedb.org/docs/rate-limiting
    /// There isn't actually any respecting of this quite yet.
    // TODO: Respect this response
    TMDBTooManyRequests, 

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
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

