use std::{
    hash::{DefaultHasher, Hasher},
    num::NonZeroUsize,
    time::Duration,
};

use lru::LruCache;

use crate::ir::DeclarationId;

use super::{
    internal::InternalFunction,
    value::{RainFunction, RainHash, Value},
};

pub struct Cache {
    storage: LruCache<CacheKey, CacheEntry>,
}

impl Cache {
    pub fn new(size: NonZeroUsize) -> Self {
        Self {
            storage: LruCache::new(size),
        }
    }

    pub fn function_key<'a>(
        &self,
        function: impl Into<FunctionDefinition>,
        args: impl Iterator<Item = &'a Value>,
    ) -> CacheKey {
        let mut hasher = DefaultHasher::new();
        for a in args {
            RainHash::hash(a, &mut hasher);
        }
        let args_hash = hasher.finish();
        CacheKey {
            definition: function.into(),
            args_hash,
        }
    }

    pub fn get_value(&mut self, key: &CacheKey) -> Option<&Value> {
        self.storage.get(key).map(|e| &e.value)
    }

    pub fn put(
        &mut self,
        key: CacheKey,
        execution_time: Duration,
        deps: Vec<CacheKey>,
        value: Value,
    ) {
        self.storage.put(
            key,
            CacheEntry {
                execution_time,
                deps,
                value,
            },
        );
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    definition: FunctionDefinition,
    args_hash: u64,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum FunctionDefinition {
    DeclarationId(DeclarationId),
    Internal(InternalFunction),
}

impl From<&RainFunction> for FunctionDefinition {
    fn from(f: &RainFunction) -> Self {
        Self::DeclarationId(f.id)
    }
}

impl From<InternalFunction> for FunctionDefinition {
    fn from(f: InternalFunction) -> Self {
        Self::Internal(f)
    }
}

#[expect(dead_code)]
struct CacheEntry {
    execution_time: Duration,
    deps: Vec<CacheKey>,
    value: Value,
}
