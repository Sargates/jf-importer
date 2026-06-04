use regex::Regex;
use serde_json::{self, to_string_pretty};
use serde::{Deserialize, Serialize};
use reqwest::{Client, Response};

use tokio::time::Instant;
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};
use governor::{Quota, RateLimiter, DefaultDirectRateLimiter};

use async_trait::async_trait;

use std::sync::Arc;
use std::cell::RefCell;
use std::time::Duration;

use crate::config::SECRETS;
use crate::media_item::{MediaItem, Movie, Show, Episode};
use crate::api::{ApiClient, QueryStatus, QueryError, QueryResponse, tmdb};

/// Matches schema from querying:
///   - https://api.themoviedb.org/3/search/movie?query=<MOVIE>&api_key=<KEY>
///   OR
///   - https://api.themoviedb.org/3/search/tv?query=<TV_SHOW>&api_key=<KEY>
/// Docs: https://developer.themoviedb.org/reference/search-movie
#[derive(Debug, Default, Deserialize)]
pub struct TMDBSearchResponse {
    /// Whether this is an adult movie
    adult: bool,

    /// Route to fetch a backdrop image for the media
    /// The value here should be appended to `https://image.tmdb.org/t/p/w1280`
    backdrop_path: String,

    /// Vector of UUIDs that TMDB uses for genres
    genre_ids: Vec<u32>,

    /// TMDB UUID for media item
    id: u32,

    /// Languages of origin
    original_language: String,

    /// Originaly title of media
    #[serde(alias = "original_name")]
    original_title: String,
    
    /// Short description of media
    overview: String,

    /// TMDB popularity of the media
    popularity: f32,

    /// Route to fetch a poster image for the media
    poster_path: String,

    /// YYYY-MM-DD
    #[serde(alias = "first_air_date")]
    release_date: String, 

    /// Porn??
    softcore: bool,

    /// Title of the media
    #[serde(alias = "name")]
    title: String,

    /// Average rating of media: [0-10]
    vote_average: f32,

    /// Number of votes
    vote_count: f32,

    // Addon things, just filler fields to ensure serde doesn't fail
    /// Whether this is a video? Like Youtube?
    #[serde(skip_serializing_if = "Option::is_none")]
    video: Option<bool>,

    /// Countries of origin
    #[serde(skip_serializing_if = "Option::is_none")]
    origin_country: Option<Vec<String>>,

    // Post-creation things that aren't included in the API; these require extra processing
    /// IMDB ID of queried item
    #[serde(skip_deserializing)]
    imdb_id: String, // needs separate API call with TMDB
}
impl TryInto<QueryResponse> for TMDBSearchResponse {
    type Error = QueryError;
    fn try_into(self) -> Result<QueryResponse, Self::Error> {
        if self.imdb_id.is_empty() {
            return Err(QueryError::TMDBIncompleteResponse)
        }
        let collection: String = self.release_date.chars().take(4).collect();
        Ok(QueryResponse {
            title: self.title,
            year: str::parse::<u32>(&collection).map_err(|_| QueryError::TMDBFailedToConvertFromResponse)?,
            imdb: self.imdb_id.is_empty().then_some(self.imdb_id),
            tmdb: format!("{}", self.id),
        })
    }
}
impl TMDBSearchResponse {
    fn from_json(object: serde_json::Value) -> Result<Self, serde_json::Error> {
        Ok(serde_json::from_value::<Self>(object)?)
    }
}

/// Matches schema from querying:
///   - https://api.themoviedb.org/3/movie/<MOVIE_ID>/external_ids?api_key=<KEY>
///   OR
///   - https://api.themoviedb.org/3/tv/<TV_SHOW_ID>/external_ids?api_key=<KEY>
/// Docs: https://developer.themoviedb.org/reference/movie-details
#[derive(Debug, Default, Deserialize)]
pub struct TMDBExternalIdsResponse {
    // TMDB id
    id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    imdb_id: Option<String>,

    // Half of these we don't care about
    #[serde(skip_serializing_if = "Option::is_none")]
    freebase_mid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    freebase_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tvdb_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tvrage_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wikidata_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    facebook_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instagram_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    twitter_id: Option<String>,
}
impl TMDBExternalIdsResponse {
    fn from_json(object: serde_json::Value) -> Result<Self, serde_json::Error> {
        Ok(serde_json::from_value::<Self>(object)?)
    }
}

pub struct TMDBClient {
    client: reqwest::Client,
    rate_limiter: DefaultDirectRateLimiter,
    key: String,
}

impl TMDBClient {
    pub fn new() -> Self {
        let rate_limiter = RateLimiter::direct(
            Quota::per_second(std::num::NonZeroU32::new(20).unwrap())
        );
        Self {
            client: Client::new(),
            rate_limiter, 
            key: SECRETS.clone().TMDB_KEY,
        }
    }
}

#[async_trait]
impl ApiClient for TMDBClient {
    // TODO: Add checking override
    async fn search_movie(&self, movie: Arc<Movie>) -> QueryStatus {
        if self.key.is_empty() { return QueryStatus::Failed(QueryError::UnsetApiKey) }
        self.rate_limiter.until_ready().await;

        // let mut guard = movie.lock().await;
        let file_stem = movie.src.file_stem().unwrap().to_str().unwrap().to_string();

        let encoded = urlencoding::encode(&file_stem);
        let url = format!("https://api.themoviedb.org/3/search/movie?query={}&api_key={}", encoded, self.key);
        // println!("Curling: {}", url);

        let response = match self.client.get(&url).send().await.map_err(|e| e.into()) {
            Ok(r) => r,
            Err(e) => return QueryStatus::Failed(e),
        };
        let json: serde_json::Value = match response.json().await.map_err(|e| e.into()) {
            Ok(r) => r,
            Err(e) => return QueryStatus::Failed(e),
        };

        // Check that we succeeded
        if json["total_results"] == 0 {
            return QueryStatus::Failed(QueryError::NoSearchResultsFromApi);
        }

        // Unwrap the `results` given from the API
        let json = json["results"][0].clone();

        // println!("Response: {:?}", json);
        let mut out_query = match TMDBSearchResponse::from_json(json).map_err(|e| e.into()) {
            Ok(r) => r,
            Err(e) => return QueryStatus::Failed(e),
        };
        // println!("HERE!");

        // Fetch actual IMDB ID
        let fut = async {
            let url = format!("https://api.themoviedb.org/3/movie/{}/external_ids?api_key={}", out_query.id, self.key);
            // println!("Curling: {}", url);
            let response = self.client.get(&url).send().await?;
            // println!("tmdb_response: {:?}", out_query);
            let json: serde_json::Value = response.json().await?;
            // println!("Response: {:?}", json);
            let tmdb_response = TMDBExternalIdsResponse::from_json(json)?;
            // println!("tmdb_response: {:?}", tmdb_response);
            out_query.imdb_id = tmdb_response.imdb_id.unwrap_or(String::new());
            Ok::<(), QueryError>(())
        };

        if let Err(err) = fut.await {
            tracing::error!("Failed to query External IDS for IMDB id!")
        }
        match out_query.try_into() {
            Ok(r) => QueryStatus::Success(r),
            Err(e) => QueryStatus::Failed(e)
        }
    }
    async fn search_show(&self, show: Arc<Show>) -> QueryStatus {
        if self.key.is_empty() { return QueryStatus::Failed(QueryError::UnsetApiKey) }
        self.rate_limiter.until_ready().await;

        // println!("Waiting for lock!");
        // let mut guard = show.lock().await;
        // println!("Acquired Lock: {:?}", guard.src);
        let file_stem = show.src.file_name().unwrap().to_str().unwrap().to_string();
        // println!("Src: {:?}, Name: {:?}", guard.src, guard.src.file_name().unwrap());

        let encoded = urlencoding::encode(&file_stem);
        let url = format!("https://api.themoviedb.org/3/search/tv?query={}&api_key={}", encoded, self.key);
        // println!("Curling: {}", url);

        // println!("Sending call for: {:?}", guard.src.file_name());
        let response = match self.client.get(&url).send().await.map_err(|e| e.into()) {
            Ok(r) => r,
            Err(e) => return QueryStatus::Failed(e),
        };
        // println!("Received call for: {:?}", guard.src.file_name());
        let json: serde_json::Value = match response.json().await.map_err(|e| e.into()) {
            Ok(r) => r,
            Err(e) => return QueryStatus::Failed(e),
        };

        // Check that we succeeded
        if json["total_results"] == 0 {
            return QueryStatus::Failed(QueryError::NoSearchResultsFromApi);
        }

        // Unwrap the `results` given from the API
        let json = json["results"][0].clone();

        // println!("Response: {:?}", json);
        let mut out_query = match TMDBSearchResponse::from_json(json).map_err(|e| e.into()) {
            Ok(r) => r,
            Err(e) => return QueryStatus::Failed(e),
        };
        // println!("Query: {:?}", out_query);

        // Fetch actual IMDB ID
        let fut = async {
            let url = format!("https://api.themoviedb.org/3/tv/{}/external_ids?api_key={}", out_query.id, self.key);
            // println!("Curling: {}", url);
            let response = self.client.get(&url).send().await?;
            // println!("tmdb_response: {:?}", out_query);
            let json: serde_json::Value = response.json().await?;
            // println!("Response: {:?}", json);
            let tmdb_response = TMDBExternalIdsResponse::from_json(json)?;
            // println!("tmdb_response: {:?}", tmdb_response);
            out_query.imdb_id = tmdb_response.imdb_id.unwrap_or(String::new());
            Ok::<(), QueryError>(())
        };

        if let Err(err) = fut.await {
            tracing::error!("Failed to query External IDS for IMDB id!")
        }
        match out_query.try_into() {
            Ok(r) => QueryStatus::Success(r),
            Err(e) => QueryStatus::Failed(e),
        }
    }
}

