use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    exec::{types::RainValue, ExecCF},
    leaf::LeafSet,
};

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct CacheKey {
    implementation_hash: u64,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub value: Result<RainValue, ExecCF>,
    pub leaves: LeafSet,
}

pub trait Cache: std::fmt::Debug {
    fn get(&self, key: &CacheKey) -> Option<CacheEntry>;
    fn put(&self, key: CacheKey, value: CacheEntry);

    fn get_option(&self, key: &Option<CacheKey>) -> Option<CacheEntry> {
        self.get(key.as_ref()?)
    }

    fn put_option(&self, key: Option<CacheKey>, entry: &CacheEntry) {
        let Some(k) = key else {
            return;
        };
        self.put(k, entry.clone());
    }
}

#[derive(Debug, Default, Clone)]
pub struct MemCache {
    mem: Arc<Mutex<HashMap<CacheKey, CacheEntry>>>,
}

impl Cache for MemCache {
    fn get(&self, _key: &CacheKey) -> Option<CacheEntry> {
        None
        // self.mem.lock().unwrap().get(key).cloned()
    }

    fn put(&self, key: CacheKey, value: CacheEntry) {
        self.mem.lock().unwrap().insert(key, value);
    }
}
