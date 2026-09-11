use std::sync::Arc;
use std::rc::Rc;
use std::pin::Pin;
use std::cell::{Ref, RefMut};

use std::collections::HashMap;
use std::time::Duration;

use futures::FutureExt;
use futures::{future::ready, Future};
use futures::future::Shared;
use futures::stream::{FuturesUnordered, Stream, StreamExt};
use tokio;
use tokio::sync::{Mutex, MutexGuard, TryLockError};

use crate::media::*;
use crate::api::client::*;
use crate::catalog::MediaItem;

pub struct ApiCallFuture {
    client: Arc<dyn ApiClient + Sync + Send>,
    inner: Pin<Box<dyn Future<Output = ApiCall> + Send>>,
}
impl ApiCallFuture {
    pub fn new(item: MediaItem, client: Arc<dyn ApiClient + Sync + Send>) -> Self {
        Self {
            client,
            inner: Box::pin(futures::future::pending())
        }.init(item)
    }
    // I don't want to deal with lifetime syntax
    fn init(mut self, item: MediaItem) -> Self {
        // *     this pattern of searching a `MediaItem` happens a lot. 
        // TODO: make this an async function wrapper that returns the resulting future
        let client = self.client.clone();
        let fut = async move {
            let status = match item.clone() {
                MediaItem::Movie(movie) => client.search_movie(movie).await,
                MediaItem::Show(show) => client.search_show(show).await,
                MediaItem::Episode(episode) => todo!(),
            };
            // TODO: remove this sleep!
            // tokio::time::sleep(Duration::from_secs(4)).await;
            ApiCall {
                item,
                status
            }
        };
        self.inner = Box::pin(fut);
        self
    }
}

impl Future for ApiCallFuture {
    type Output = ApiCall;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        self.inner.poll_unpin(cx)
    }
}

/// Data-only object for call to an API
pub struct ApiCall {
    pub item: MediaItem,
    pub status: QueryStatus,
}

#[derive(Default)]
pub struct ApiManifest {
    // api_count: u32, // TODO: support tracking how many API calls are made for a single Future<T>
    outgoing: FuturesUnordered<ApiCallFuture>,
    //? Does this have to be in a mutex? I don't see anything immediately that says it does
    api_results: HashMap<MediaItem, Arc<QueryStatus>>,
    released: bool, // send all api calls
}
impl ApiManifest {
    pub fn new() -> Self {
        ApiManifest {
            outgoing: FuturesUnordered::new(),
            api_results: HashMap::new(),
            released: false
        }
    }
    pub fn push_future(&mut self, f: ApiCallFuture) { self.outgoing.push(f); }
    pub fn is_empty(&self) -> bool                  { self.outgoing.is_empty() }
    pub fn send(&mut self) -> ()                    { self.released = true; }
    pub async fn next(&mut self) -> Option<ApiCall> {
        if self.released { self.outgoing.next().await } 
        else             { None }
    }
    pub fn update_api_call(&mut self, call: ApiCall) {
        match &call.status {
            QueryStatus::Success(response) => tracing::info!(
                "[Success] Result: src -> {:?}",
                format!(
                    "{} ({}) [tmdbid-{}]",
                    response.title,
                    response.year,
                    match &response.tmdb {
                        ApiSpecificMediaId::TMDB(id) => id,
                        ApiSpecificMediaId::OMDB(id) => id,
                    }
                )),
            QueryStatus::Failed(query_error) => tracing::error!(
                "[Failure] Failed to query API: {:?}",
                query_error),
            _ => {}
            // we use this method for the initial insertion into the hashmap,
            // so NotStarted will display nothing if used for that.
        }
        self.api_results.insert(call.item, Arc::new(call.status));
    }

    /// Returns a reference to the `Arc<T>` value inside the hashmap.
    /// Mainly to support bindings living long enough in specific situations.
    pub fn get(&self, item: &MediaItem) -> Option<&Arc<QueryStatus>> {
        self.api_results.get(&item)
    }
}

impl Stream for ApiManifest {
    type Item = ApiCall;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        self.outgoing.poll_next_unpin(cx)
    }
}
