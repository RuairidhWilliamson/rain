use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{exec::types::RainValue, leaf::LeafSet};

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct FunctionCallCacheKey {
    implementation_hash: u64,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub value: RainValue,
    pub leaves: LeafSet,
}

pub trait Cache: std::fmt::Debug {
    fn get(&self, key: &FunctionCallCacheKey) -> Option<CacheEntry>;
    fn put(&self, key: FunctionCallCacheKey, value: CacheEntry);
}

#[derive(Debug, Default, Clone)]
pub struct MemCache {
    mem: Arc<Mutex<HashMap<FunctionCallCacheKey, CacheEntry>>>,
}

impl Cache for MemCache {
    fn get(&self, _key: &FunctionCallCacheKey) -> Option<CacheEntry> {
        None
        // self.mem.lock().unwrap().get(key).cloned()
    }

    fn put(&self, key: FunctionCallCacheKey, value: CacheEntry) {
        self.mem.lock().unwrap().insert(key, value);
    }
}
