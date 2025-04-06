pub mod persistent;

use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use lru::LruCache;
use poison_panic::MutexExt as _;
use rain_lang::runner::{
    cache::{CacheEntry, CacheKey},
    dep::Dep,
};

pub const CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(1024).expect("cache size must be non zero");

#[derive(Default, Clone)]
pub struct Cache(pub Arc<Mutex<CacheCore>>);

impl Cache {
    pub fn new(core: CacheCore) -> Self {
        Self(Arc::new(Mutex::new(core)))
    }

    pub fn len(&self) -> usize {
        self.0.plock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.plock().is_empty()
    }
}

impl rain_lang::runner::cache::CacheTrait for Cache {
    fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        if !key.pure() {
            return None;
        }
        let mut guard = self.0.plock();
        guard.storage.get(key).cloned()
    }

    fn put(&self, key: CacheKey, entry: CacheEntry) {
        if !key.pure() {
            return;
        }
        if entry.deps.iter().any(|d| matches!(d, Dep::Uncacheable)) {
            return;
        }
        self.0.plock().storage.put(key, entry);
    }

    fn inspect_all(&self) -> Vec<String> {
        self.0
            .plock()
            .storage
            .iter()
            .map(|(k, v)| {
                let mut s = format!("{k} => {:?} {:?}", v.value, v.execution_time);
                if s.len() > 200 {
                    s.truncate(197);
                    s.push_str("...");
                }
                s
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct CacheCore {
    storage: LruCache<CacheKey, CacheEntry>,
}

impl Default for CacheCore {
    fn default() -> Self {
        Self::new(CACHE_SIZE)
    }
}

impl CacheCore {
    pub fn new(cap: NonZeroUsize) -> Self {
        Self {
            storage: LruCache::new(cap),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    pub fn len(&self) -> usize {
        self.storage.len()
    }
}
