use std::collections::HashMap;
use super::{QueryResponse, QueryError};
use crate::media_item::{MediaItem, Movie, Show, Episode};

struct ApiStore {
    store: HashMap<MediaItem, QueryResponse>
}
impl ApiStore {
    pub fn get(&self, item: MediaItem) -> Option<&QueryResponse> {
        self.store.get(&item)
    }
    pub fn insert(&mut self, tuple: (MediaItem, QueryResponse)) {
        self.store.insert(tuple.0, tuple.1);
    }
}
