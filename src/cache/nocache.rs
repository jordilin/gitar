use crate::cache::{Cache, CacheState};
use crate::io::{Response, ResponseField};

use crate::Result;

pub struct NoCache;

impl<K> Cache<K> for NoCache {
    fn get(&self, _key: &K) -> Result<CacheState> {
        Ok(CacheState::None)
    }
    fn set(&self, _key: &K, _value: &Response) -> Result<()> {
        Ok(())
    }

    fn update(&self, _key: &K, _value: &Response, _field: &ResponseField) -> Result<()> {
        Ok(())
    }
}
