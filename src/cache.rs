use crate::io::Response;

pub mod filesystem;
pub mod inmemory;
pub mod nocache;

use crate::Result;
pub use inmemory::InMemoryCache;
pub use nocache::NoCache;

pub trait Cache {
    fn get(&self, key: &str) -> Result<CacheState>;
    fn set(&self, key: &str, value: &Response) -> Result<()>;
}

pub enum CacheState {
    Stale(Response),
    Fresh(Response),
    None,
}
