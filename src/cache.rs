use crate::io::{HttpResponse, ResponseField};

pub mod filesystem;
pub mod inmemory;
pub mod nocache;

use crate::Result;
pub use inmemory::InMemoryCache;
pub use nocache::NoCache;

pub trait Cache<K = String> {
    fn get(&self, key: &K) -> Result<CacheState>;
    fn set(&self, key: &K, value: &HttpResponse) -> Result<()>;
    fn update(&self, key: &K, value: &HttpResponse, field: &ResponseField) -> Result<()>;
}

pub enum CacheState {
    Stale(HttpResponse),
    Fresh(HttpResponse),
    None,
}
