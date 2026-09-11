use std::rc::Rc;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;

use itertools::Itertools;

use futures::future;
use tokio::sync::{Mutex, MutexGuard, TryLockError};

use crate::media::*;
use crate::api::{
    calls::*,
    client::*
};

mod catalog_item;
mod catalog_builder;

pub use catalog_item::*;
pub use catalog_builder::*;

use crate::config::{Config,Secrets,SECRETS};


pub struct Catalog {
    pub(crate) config: Arc<Config>,
    pub(crate) movies: Vec<Arc<Movie>>,
    pub(crate) shows: Vec<Arc<Show>>,
    pub(crate) episodes: Vec<Arc<Episode>>,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) api_manifest: Mutex<ApiManifest>,
    pub(crate) api_client: Option<Arc<dyn ApiClient + Sync + Send>>,
}

impl Catalog {
    /// Return an iterator of all `Movie`s
    pub fn iter_movies(&self) -> impl Iterator<Item = Arc<Movie>> {
        self.movies.iter().map(|m| m.clone())
    }

    /// Return an iterator of all `Show`s
    pub fn iter_shows(&self) -> impl Iterator<Item = Arc<Show>> {
        self.shows.iter().map(|s| s.clone())
    }

    /// Return an iterator of all `Movie` and `Show` objects
    pub fn iter_all(&self) -> impl Iterator<Item = MediaItem> {
        let movies = self.movies.iter()
            .map(|m| MediaItem::Movie(m.clone()));
        let shows = self.shows.iter()
            .map(|s| MediaItem::Show(s.clone()));
        movies.chain(shows)
    }

    /// Return a sorted iterator of all `Movie` and `Show` objects.
    ///
    /// We sort Movies independently and then Shows independently 
    /// and then concatenated to preserve contiguity. 
    pub fn iter_all_sorted(&self) -> impl Iterator<Item = MediaItem> {
        // This is actually so fucking concise compared to the first draft of this 
        // function. Rust's typing system can be very annoying, but so beautiful 
        // sometimes.
        //
        // As a note, you can't do conditional mappings without putting the logic
        // inside of the closures or you get incompatible return types for the
        // method. You could technically `collect` the sorted iterator, but
        // then you'd be returning a whole new vector. Even if you could `collect`
        // and re-`iter`, it still would clone the old vector and force you to
        // manage the lifetime of the `collect` anyways. `collect`ing at all also
        // defeats the purpose of sorting the iterators in place to avoid
        // unneccessary memory allocations
        
        // we acquire the lock before sorting just to be concise. otherwise we'd
        // have to inline-acquire the lock with `if let` and return `clone`'d sort
        // keys since the lock would be dropped and the reference invalidated.

        // use Option<T> to be concise with `if let`
        // seamlessly falls back upon failure to acquire the lock
        let lock = self.api_manifest.try_lock().ok();

        // if the given element in `movies` or `shows` has a successfully-resolved 
        // API query, use the title from the response as a sort key, otherwise,
        // fall back to the `search_term` member.
        let movies = self.movies.iter()
            .sorted_by(|a,b| { //? Why are a and b `&&Arc<T>`? `.iter` on `&self` member?
                // oh my fucking god this chaining of `if let` is so beautiful.
                // the first draft of this code was so dogshit compared to this.
                let string_a = if let Some(ref lock) = lock &&
                                  let Some(rc) = lock.get(&MediaItem::Movie((*a).clone())) &&
                                  let QueryStatus::Success(response_a) = rc.as_ref() {
                       &response_a.title }
                else { &a.search_term };
                let string_b = if let Some(ref lock) = lock &&
                                  let Some(rc) = lock.get(&MediaItem::Movie((*b).clone())) &&
                                  let QueryStatus::Success(response_b) = rc.as_ref() {
                       &response_b.title }
                else { &b.search_term };

                Ord::cmp(string_a, string_b)
            })
            .map(|m| MediaItem::Movie(m.clone()));

        let shows = self.shows.iter()
            .sorted_by(|a,b| {
                let string_a = if let Some(ref lock) = lock &&
                                  let Some(rc) = lock.get(&MediaItem::Show((*a).clone())) &&
                                  let QueryStatus::Success(response_a) = rc.as_ref() {
                       &response_a.title }
                else { &a.search_term };
                let string_b = if let Some(ref lock) = lock &&
                                  let Some(rc) = lock.get(&MediaItem::Show((*b).clone())) &&
                                  let QueryStatus::Success(response_b) = rc.as_ref() {
                       &response_b.title }
                else { &b.search_term };

                Ord::cmp(string_a, string_b)
            })
            .map(|m| MediaItem::Show(m.clone()));

        movies.chain(shows)
    }

    // TODO: this shit needs to be rewritten. I can't think of the best relationship between the
    //       ApiManifest and the Catalog right now.
    pub fn try_lock_manifest<'a>(&'a self) -> Result<MutexGuard<'a, ApiManifest>, TryLockError> { self.api_manifest.try_lock() }
    pub async fn lock<'a>(&'a self) -> MutexGuard<'a, ApiManifest> { self.api_manifest.lock().await }

    pub fn stage_api_calls(&mut self) {
        if let Some(ref client) = self.api_client {
            for media in self.iter_all() {
                let mut lock = self.api_manifest.try_lock().unwrap();
                let future = ApiCallFuture::new(media.clone(), client.clone());
                lock.push_future(future);

                // let fake_call = ApiCall { item: media.clone(), status: QueryStatus::NotStarted };
                // lock.update_api_call(fake_call);
            }
        }
        // no need to check `api_client` here, `futs` will be empty otherwise.
    }
}

