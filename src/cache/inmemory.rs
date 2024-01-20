use std::{cell::RefCell, collections::HashMap};

use crate::{
    cache::{Cache, CacheState},
    http::Resource,
    io::{Response, ResponseField},
};

use crate::Result;

pub struct InMemoryCache {
    cache: RefCell<HashMap<String, Response>>,
    expired: bool,
    pub updated: RefCell<bool>,
    pub updated_field: RefCell<ResponseField>,
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
            expired: false,
            updated: RefCell::new(false),
            // This is to verify we will get the headers updated in a 304
            // response. Set it to Body, so we can verify headers are actually
            // updated during tests. Might be better to have a builder pattern
            // for this (TODO)
            updated_field: RefCell::new(ResponseField::Body),
        }
    }
}

impl InMemoryCache {
    pub fn expire(&mut self) {
        self.expired = true;
    }
}

impl Cache<Resource> for &InMemoryCache {
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

    fn update(
        &self,
        key: &Resource,
        value: &Response,
        field: &crate::io::ResponseField,
    ) -> Result<()> {
        *self.updated.borrow_mut() = true;
        *self.updated_field.borrow_mut() = field.clone();
        self.set(key, value)
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

    fn update(
        &self,
        key: &String,
        value: &Response,
        _field: &crate::io::ResponseField,
    ) -> Result<()> {
        self.set(key, value)
    }
}
