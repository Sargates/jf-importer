use std::slice::Iter;
use std::sync::Arc;

use crate::media::{Episode, MediaItem, Movie, Show, CatalogFailure};
use crate::api::{
    calls::ApiManifest,
    client::{QueryResponse, QueryStatus}
};
use crate::config::{Config,CONFIG,Secrets,SECRETS};

pub struct Catalog {
    pub(crate) config: Config,
    pub(crate) movies: Vec<Arc<Movie>>,
    pub(crate) shows: Vec<Arc<Show>>,
    pub(crate) episodes: Vec<Arc<Episode>>,
    pub(crate) failures: Vec<CatalogFailure>,
    // pub api_manifest: ApiManifest,
}

impl Catalog {
    /// Return an iterator of all `Movie`
    pub fn iter_movies(&self) -> impl Iterator<Item = Arc<Movie>> {
        self.movies.iter().map(|m| m.clone())
    }
    pub fn iter_shows(&self) -> impl Iterator<Item = Arc<Show>> {
        self.shows.iter().map(|s| s.clone())
    }
    // /// Number of `Movie` and `Show` objects that were successfully added to the catalog
    // pub fn len(&self) -> usize {
    //     self.movies.len() + self.shows.len()
    // }
    /// Return an interator of all `Movie` and `Show` objects
    pub fn iter_all(&self) -> impl Iterator<Item = MediaItem> {
        let movies = self.movies.iter()
            .map(|m| MediaItem::Movie(m.clone()));
        let shows = self.shows.iter()
            .map(|s| MediaItem::Show(s.clone()));
        movies.chain(shows)
    }
}

