use crate::cache::{Cache, CacheState};
use crate::io::Response;

use crate::Result;

pub struct NoCache;

impl Cache for NoCache {
    fn get(&self, _key: &str) -> Result<CacheState> {
        Ok(CacheState::None)
    }
    fn set(&self, _key: &str, _value: &Response) -> Result<()> {
        Ok(())
    }
}
