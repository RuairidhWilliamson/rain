use std::{
    fmt::Display,
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use lru::LruCache;
use poison_panic::MutexExt as _;

use crate::ir::DeclarationId;

use super::{dep::Dep, internal::InternalFunction, value::Value};

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

    pub fn save(&self, path: &Path) {
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
            inner: serde_json::to_value(p).unwrap(),
        };
        let serialized = serde_json::to_vec_pretty(&p).unwrap();
        std::fs::write(path, serialized).unwrap();
    }

    pub fn load(&self, path: &Path) {
        let mut storage = self.storage.plock();
        let Ok(serialized) = std::fs::read(path) else {
            log::debug!("persistent cache did not exist");
            return;
        };
        let PersistentCacheWrapper {
            format_version,
            inner,
        }: PersistentCacheWrapper = serde_json::from_slice(&serialized).unwrap();
        if format_version != FORMAT_VERSION {
            panic!("bad format version");
        }
        let p: PersistentCache = serde_json::from_value(inner).unwrap();
        for (url, v) in p.downloads {
            storage.push(CacheKey::Download { url }, v);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.storage.plock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.storage.plock().len()
    }

    pub fn get_value(&self, key: &CacheKey) -> Option<Value> {
        if !key.pure() {
            return None;
        }
        self.storage.plock().get(key).map(|e| e.value.clone())
    }

    pub fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        if !key.pure() {
            return None;
        }
        let mut guard = self.storage.plock();
        guard.get(key).cloned()
    }

    pub fn put(
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

    pub fn inspect_all(&self) -> Vec<String> {
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

fn display_vec<T: Display>(v: &Vec<T>) -> String {
    let mut s = String::new();
    let mut first = true;
    for e in v {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&e.to_string());
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKey {
    Declaration {
        declaration: DeclarationId,
        args: Vec<Value>,
    },
    InternalFunction {
        func: InternalFunction,
        args: Vec<Value>,
    },
    Download {
        url: String,
    },
}

impl CacheKey {
    fn pure(&self) -> bool {
        match self {
            Self::InternalFunction { func: _, args }
            | Self::Declaration {
                declaration: _,
                args,
            } => args.iter().all(Value::cache_pure),
            Self::Download { url: _ } => true,
        }
    }
}

impl Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declaration { declaration, args } => {
                f.write_fmt(format_args!("{declaration}({})", display_vec(args)))
            }
            Self::InternalFunction { func, args } => {
                f.write_fmt(format_args!("{func}({})", display_vec(args)))
            }
            Self::Download { url } => f.write_fmt(format_args!("Download({url})")),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheEntry {
    pub execution_time: Duration,
    pub expires: Option<DateTime<Utc>>,
    pub etag: Option<String>,
    pub deps: Vec<Dep>,
    pub value: Value,
}

pub enum CacheStrategy {
    Always,
    Never,
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
