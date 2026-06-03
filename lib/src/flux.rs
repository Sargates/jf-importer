// Trying to mimic the [Flux Architecture](https://ratatui.rs/concepts/application-patterns/flux-architecture/)

use once_cell::sync::Lazy;

pub static DISPATCH: Lazy<Dispatch> = Lazy::new(|| {
    Dispatch::new()
});
pub static UNSPECIFIC_STORE: Lazy<UnspecificStore> = Lazy::new(|| {
    UnspecificStore::new()
});

pub struct Dispatch {}
impl Dispatch {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct UnspecificStore {}
impl UnspecificStore {
    pub fn new() -> Self {
        Self {}
    }
}
