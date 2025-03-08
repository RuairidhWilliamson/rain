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

pub const CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(1024).expect("cache size must be non zero");

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

    pub fn is_empty(&self) -> bool {
        self.storage.plock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.storage.plock().len()
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
        if value.storeable() {
            self.storage.plock().put(
                key,
                CacheEntry {
                    execution_time,
                    expires: None,
                    value,
                },
            );
        } else {
            log::debug!(
                "attempted to store {:?} in cache but it is not storeable",
                value.rain_type_id()
            );
        }
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
