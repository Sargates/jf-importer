// I am tired of dealing with how to get what I want, where I want, while dealing with multithreaded bullshit;
// global state makes this easy. This decision was not made lightly, I agonized over it.
use std::sync::Arc;

use dashmap::{DashMap, mapref::one::Ref}; // glorious crate
use once_cell::sync::Lazy;

use crate::media_item::MediaItem;
use crate::api::QueryStatus;


pub static API_CALLS: Lazy<Arc<Globals>> = Lazy::new(|| {
    Arc::new(Default::default())
});


#[derive(Default)]
pub struct Globals {
    api_calls: DashMap<MediaItem, QueryStatus>,
}
impl Globals {
    pub fn push_query(&self, key: MediaItem, value: QueryStatus) -> Option<QueryStatus> {
        self.api_calls.insert(key, value)
    }
    pub fn get_query(&self, key: &MediaItem) -> Option<Ref<'_, MediaItem, QueryStatus>> {
        self.api_calls.get(key)
    }
}
