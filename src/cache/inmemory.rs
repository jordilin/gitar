use std::{cell::RefCell, collections::HashMap};

use crate::{
    cache::{Cache, CacheState},
    io::Response,
};

use crate::Result;

#[derive(Default)]
pub struct InMemoryCache {
    cache: RefCell<HashMap<String, Response>>,
    expired: bool,
}

impl InMemoryCache {
    pub fn expire(&mut self) {
        self.expired = true;
    }
}

impl Cache for InMemoryCache {
    fn get(&self, key: &str) -> Result<CacheState> {
        if let Some(response) = self.cache.borrow().get(key) {
            if self.expired {
                return Ok(CacheState::Stale(response.clone()));
            }
            return Ok(CacheState::Fresh(response.clone()));
        }
        Ok(CacheState::None)
    }

    fn set(&self, key: &str, value: &Response) -> Result<()> {
        self.cache
            .borrow_mut()
            .insert(key.to_string(), value.clone());
        Ok(())
    }
}
