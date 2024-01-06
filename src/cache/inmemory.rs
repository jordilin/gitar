use std::{cell::RefCell, collections::HashMap};

use crate::{
    cache::{Cache, CacheState},
    http::Resource,
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

impl Cache<Resource> for InMemoryCache {
    fn get(&self, key: &Resource) -> Result<CacheState> {
        if let Some(response) = self.cache.borrow().get(&key.url) {
            if self.expired {
                return Ok(CacheState::Stale(response.clone()));
            }
            return Ok(CacheState::Fresh(response.clone()));
        }
        Ok(CacheState::None)
    }

    fn set(&self, key: &Resource, value: &Response) -> Result<()> {
        self.cache
            .borrow_mut()
            .insert(key.url.to_string(), value.clone());
        Ok(())
    }
}

impl Cache<String> for InMemoryCache {
    fn get(&self, key: &String) -> Result<CacheState> {
        if let Some(response) = self.cache.borrow().get(key) {
            if self.expired {
                return Ok(CacheState::Stale(response.clone()));
            }
            return Ok(CacheState::Fresh(response.clone()));
        }
        Ok(CacheState::None)
    }

    fn set(&self, key: &String, value: &Response) -> Result<()> {
        self.cache
            .borrow_mut()
            .insert(key.to_string(), value.clone());
        Ok(())
    }
}
