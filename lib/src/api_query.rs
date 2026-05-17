use reqwest::blocking::{
    Client,
    Request, RequestBuilder,
    Response,
};

use crate::media_item::{Movie, Show, Episode, Mappable, MappingError};

#[derive(Debug, Clone)]
pub enum ApiError {
    FailedToQueryApi
    // TODO: Reference actual API responses
    //? Create unified enum for different APIs?
}

enum QueryResponse {
    Movie {
        title: String,
        year: String,
        imdb: String,
    },
    Show {
        title: String,
        start_year: String,
        tmdb: String,
    },
    Episode {
        id: String,
    },
}
// pub fn query_tmbd_movie() -> Result<QueryResponse, ApiError> {
//     Err(ApiError::FailedToQueryApi)
// }
pub trait Queryable: Sized {
    type Error;
    fn query_api(self) -> Result<Self,Self::Error>;
}
impl Queryable for Movie {
    type Error = ApiError;
    fn query_api(self) -> Result<Movie,Self::Error> {
        todo!()
    }
}
impl Queryable for Show {
    type Error = ApiError;
    fn query_api(self) -> Result<Show,Self::Error> {
        todo!()
    }
}
impl Queryable for Episode {
    type Error = ApiError;
    fn query_api(self) -> Result<Episode,Self::Error> {
        Ok(self)
    }
}
