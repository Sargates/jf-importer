use std::rc::Rc;
use std::sync::Arc;
use std::pin::Pin;
use std::cell::{Ref, RefMut};

// use dashmap::DashMap;
use std::collections::HashMap;
use std::time::Duration;

use futures::FutureExt;
use futures::{future::ready, Future};
use futures::future::Shared;
use futures::stream::{FuturesUnordered, Stream, StreamExt};
use tokio;
use tokio::sync::{Mutex, MutexGuard, TryLockError};

use crate::media::*;
use crate::api::client::{ApiClient, QueryStatus, QueryResponse};
use crate::media::types::MediaItem;

pub struct ApiCallFuture {
    client: Arc<dyn ApiClient + Sync + Send>,
    _inner: Pin<Box<dyn Future<Output = ApiCall> + Send>>,
}
impl ApiCallFuture {
    pub fn new(item: MediaItem, client: Arc<dyn ApiClient + Sync + Send>) -> Self {
        Self {
            client,
            _inner: Box::pin(futures::future::pending()) // this is going to be really annoying to debug if it causes a deadlock. I am going to forget about this line
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
            tokio::time::sleep(Duration::from_secs(4)).await;
            ApiCall {
                item,
                status
            }
        };
        self._inner = Box::pin(fut);
        self
    }
}
impl Future for ApiCallFuture {
    type Output = ApiCall;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        self._inner.poll_unpin(cx)
    }
}
// Data-only object for call to an API
pub struct ApiCall {
    pub item: MediaItem,
    pub status: QueryStatus,
}

pub struct ApiManifest {
    // api_count: u32, // TODO: support tracking how many API calls are made for a single Future<T>
    outgoing: FuturesUnordered<ApiCallFuture>,
    api_results: Mutex<HashMap<MediaItem, Rc<QueryStatus>>>,
}
impl ApiManifest {
    pub fn new() -> Self {
        ApiManifest {
            outgoing: FuturesUnordered::new(),
            api_results: Mutex::new(HashMap::new())
        }
    }
    pub fn try_lock<'a>(&'a self) -> Result<MutexGuard<'a, HashMap<MediaItem, Rc<QueryStatus>>>, TryLockError> { self.api_results.try_lock() }
    pub async fn lock<'a>(&'a self) -> MutexGuard<'a, HashMap<MediaItem, Rc<QueryStatus>>> { self.api_results.lock().await }
    pub fn push_future(&mut self, f: ApiCallFuture) { self.outgoing.push(f); }
    // pub fn push_result(&mut self, item: MediaItem, status: QueryStatus)   { self.api_results.insert(item, status); }
    pub async fn next(&mut self) -> Option<ApiCall>                       { self.outgoing.next().await }
    pub fn is_empty(&self) -> bool                                        { self.outgoing.is_empty() }
    // pub fn get_map(&self) -> &DashMap<MediaItem, QueryStatus>             { &self.api_results }
    // pub fn get_map_mut(&mut self) -> &mut DashMap<MediaItem, QueryStatus> { &mut self.api_results }
}

impl Stream for ApiManifest {
    type Item = ApiCall;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        self.outgoing.poll_next_unpin(cx)
    }
}
