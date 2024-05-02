use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::exec::types::RainValue;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct FunctionCallCacheKey {
    implementation_hash: u64,
}

pub trait Cache: std::fmt::Debug {
    fn get(&self, key: &FunctionCallCacheKey) -> Option<RainValue>;
    fn put(&self, key: FunctionCallCacheKey, value: RainValue);
}

#[derive(Debug, Default, Clone)]
pub struct MemCache {
    mem: Arc<Mutex<HashMap<FunctionCallCacheKey, RainValue>>>,
}

impl Cache for MemCache {
    fn get(&self, key: &FunctionCallCacheKey) -> Option<RainValue> {
        self.mem.lock().unwrap().get(key).cloned()
    }

    fn put(&self, key: FunctionCallCacheKey, value: RainValue) {
        self.mem.lock().unwrap().insert(key, value);
    }
}
