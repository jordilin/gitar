use crate::io::Response;

pub mod filesystem;
pub mod inmemory;
pub mod nocache;

use crate::Result;
pub use inmemory::InMemoryCache;
pub use nocache::NoCache;

pub trait Cache<K = String> {
    fn get(&self, key: &K) -> Result<CacheState>;
    fn set(&self, key: &K, value: &Response) -> Result<()>;
}

pub enum CacheState {
    Stale(Response),
    Fresh(Response),
    None,
}
