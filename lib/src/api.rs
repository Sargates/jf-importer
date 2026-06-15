pub mod error;

mod tmdb;
mod api_common;
mod api_call;

pub mod calls {
    pub use super::api_call::*;
}
pub mod client {
    pub use super::api_common::*;
    pub use super::tmdb::*;
    // pub use super::omdb::*;
}

