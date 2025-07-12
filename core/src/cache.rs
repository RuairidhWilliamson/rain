pub mod persistent;

use std::{
    collections::HashSet,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use lru::LruCache;
use poison_panic::MutexExt as _;
use rain_lang::{
    afs::area::FileArea,
    runner::cache::{CacheEntry, CacheKey},
};

const CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(1024).expect("cache size must be non zero");
/// Minimum execution time to be stored in the cache
const EXECUTION_TIME_THRESHOLD: Duration = Duration::from_millis(1);

#[derive(Default, Clone)]
pub struct Cache {
    pub core: Arc<Mutex<CacheCore>>,
    pub stats: Arc<CacheStats>,
}

impl Cache {
    pub fn new(core: CacheCore) -> Self {
        Self {
            core: Arc::new(Mutex::new(core)),
            stats: Arc::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.core.plock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.core.plock().is_empty()
    }
}

impl rain_lang::runner::cache::CacheTrait for Cache {
    fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        if !key.pure() {
            log::debug!("cache get failed because it is not pure {key:?}");
            self.stats.get_impure.inc();
            return None;
        }
        let mut guard = self.core.plock();
        let res = guard.storage.get(key).cloned();
        if res.is_some() {
            self.stats.hits.inc();
            log::trace!("cache get hit {key:?}");
        } else {
            self.stats.misses.inc();
            log::debug!("cache get miss {key:?}");
        }
        res
    }

    fn put(&self, key: CacheKey, entry: CacheEntry) {
        if !key.pure() {
            log::debug!("not caching {key:?} because it is not pure");
            self.stats.put_fails.inc();
            return;
        }
        if entry.deps.iter().any(|d| !d.is_intra_run_stable()) {
            log::debug!(
                "not caching {key:?} because it has intra run unstable deps {entry_deps:?}",
                entry_deps = entry.deps
            );
            self.stats.put_fails.inc();
            return;
        }
        log::trace!("caching {key:?}");
        self.stats.puts.inc();
        self.core.plock().storage.put(key, entry);
    }

    fn put_if_slow(&self, key: CacheKey, entry: CacheEntry) {
        if entry.execution_time < EXECUTION_TIME_THRESHOLD {
            log::trace!(
                "not caching {key:?} because it is too fast {:?}",
                entry.execution_time,
            );
            return;
        }
        self.put(key, entry);
    }

    fn inspect_all(&self) -> Vec<String> {
        self.core
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

    fn clean(&self) {
        self.core.plock().storage.clear();
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

    pub fn get_all_generated_areas(&self) -> HashSet<&rain_lang::afs::area::GeneratedFileArea> {
        let mut out = HashSet::new();
        for (_, entry) in &self.storage {
            for area in entry.value.find_areas() {
                if let FileArea::Generated(generated_file_area) = area {
                    out.insert(generated_file_area);
                }
            }
        }
        out
    }
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: Counter,
    pub misses: Counter,
    pub get_impure: Counter,
    pub puts: Counter,
    pub put_fails: Counter,
    pub depersists: Counter,
    pub depersist_fails: Counter,
    pub persists: Counter,
    pub persist_fails: Counter,
}

#[derive(Default)]
pub struct Counter(pub AtomicUsize);

impl Counter {
    pub fn inc(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

impl std::fmt::Debug for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}
