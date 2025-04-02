use std::{
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use lru::LruCache;
use poison_panic::MutexExt as _;
use rain_lang::runner::{
    cache::{CacheEntry, CacheKey},
    dep::Dep,
    value::Value,
};

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

    pub fn save(&self, path: &Path) -> Result<(), CacheError> {
        let storage = self.storage.plock();
        let mut downloads = Vec::new();
        for (k, e) in storage.iter() {
            match k {
                CacheKey::InternalFunction { .. } | CacheKey::Declaration { .. } => (),
                CacheKey::Download { url } => downloads.push((url.to_owned(), e.clone())),
            }
        }
        let p = PersistentCache { downloads };
        let p = PersistentCacheWrapper {
            format_version: FORMAT_VERSION,
            inner: serde_json::to_value(p)?,
        };
        let serialized = serde_json::to_vec_pretty(&p)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }

    pub fn load(&self, path: &Path) -> Result<(), CacheError> {
        let mut storage = self.storage.plock();
        let Ok(serialized) = std::fs::read(path) else {
            log::debug!("persistent cache did not exist");
            return Ok(());
        };
        let PersistentCacheWrapper {
            format_version,
            inner,
        }: PersistentCacheWrapper = serde_json::from_slice(&serialized)?;
        if format_version != FORMAT_VERSION {
            return Err(CacheError::FormatVersionMissmatch);
        }
        let p: PersistentCache = serde_json::from_value(inner)?;
        for (url, v) in p.downloads {
            storage.push(CacheKey::Download { url }, v);
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.storage.plock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.storage.plock().len()
    }
}

impl rain_lang::runner::cache::CacheTrait for Cache {
    fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        if !key.pure() {
            return None;
        }
        let mut guard = self.storage.plock();
        guard.get(key).cloned()
    }

    fn put(
        &self,
        key: CacheKey,
        execution_time: Duration,
        etag: Option<String>,
        deps: &[Dep],
        value: Value,
    ) {
        if !key.pure() {
            return;
        }
        if deps.iter().any(|d| matches!(d, Dep::Uncacheable)) {
            return;
        }
        if value.storeable() {
            self.storage.plock().put(
                key,
                CacheEntry {
                    execution_time,
                    expires: None,
                    etag,
                    deps: deps.to_vec(),
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

    fn inspect_all(&self) -> Vec<String> {
        self.storage
            .plock()
            .iter()
            .map(|(k, v)| {
                let mut s = format!("{k} => {} {:?}", v.value, v.execution_time);
                if s.len() > 200 {
                    s.truncate(197);
                    s.push_str("...");
                }
                s
            })
            .collect()
    }
}

const FORMAT_VERSION: u64 = 0;

#[derive(serde::Serialize, serde::Deserialize)]
struct PersistentCacheWrapper {
    pub format_version: u64,
    pub inner: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PersistentCache {
    /// Map keyed by urls
    pub downloads: Vec<(String, CacheEntry)>,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("format missmatch")]
    FormatVersionMissmatch,
    #[error("does not exist")]
    DoesNotExist,
}
