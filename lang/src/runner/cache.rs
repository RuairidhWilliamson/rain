use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use lru::LruCache;
use poison_panic::MutexExt as _;

use crate::ir::DeclarationId;

use super::{internal::InternalFunction, value::Value, value_impl::RainFunction};

#[expect(unsafe_code)]
// Safety:
// The number is bigger than zero
pub const CACHE_SIZE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1024) };

#[derive(Clone)]
pub struct Cache {
    storage: Arc<Mutex<LruCache<CacheKey, CacheEntry>>>,
}

impl Cache {
    pub fn new(size: NonZeroUsize) -> Self {
        Self {
            storage: Arc::new(Mutex::new(LruCache::new(size))),
        }
    }

    pub fn function_key(
        &self,
        function: impl Into<FunctionDefinition>,
        args: Vec<Value>,
    ) -> CacheKey {
        CacheKey {
            definition: function.into(),
            args,
        }
    }

    pub fn get_value(&mut self, key: &CacheKey) -> Option<Value> {
        self.storage.plock().get(key).map(|e| e.value.clone())
    }

    pub fn put(&mut self, key: CacheKey, execution_time: Duration, value: Value) {
        self.storage.plock().put(
            key,
            CacheEntry {
                execution_time,
                expires: None,
                value,
            },
        );
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    definition: FunctionDefinition,
    args: Vec<Value>,
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

struct CacheEntry {
    #[expect(dead_code)]
    execution_time: Duration,
    #[expect(dead_code)]
    expires: Option<DateTime<Utc>>,
    value: Value,
}
