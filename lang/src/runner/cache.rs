use std::{
    fmt::Display,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use lru::LruCache;
use poison_panic::MutexExt as _;

use crate::ir::DeclarationId;

use super::{value::Value, value_impl::RainFunction};

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

    pub fn function_key(&self, function: impl Into<CacheKeyTarget>, args: Vec<Value>) -> CacheKey {
        CacheKey {
            target: function.into(),
            args,
        }
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

    pub fn put(&self, key: CacheKey, execution_time: Duration, etag: Option<String>, value: Value) {
        if !key.pure() {
            return;
        }
        if value.storeable() {
            self.storage.plock().put(
                key,
                CacheEntry {
                    execution_time,
                    expires: None,
                    etag,
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
                let mut s = format!(
                    "{}({}) => {} {:?}",
                    k.target,
                    display_vec(&k.args),
                    v.value,
                    v.execution_time
                );
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
pub struct CacheKey {
    pub target: CacheKeyTarget,
    pub args: Vec<Value>,
}

impl CacheKey {
    fn pure(&self) -> bool {
        self.args.iter().all(Value::cache_pure)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKeyTarget {
    DeclarationId(DeclarationId),
    Download(String),
}

impl From<&RainFunction> for CacheKeyTarget {
    fn from(f: &RainFunction) -> Self {
        Self::DeclarationId(f.id)
    }
}

impl Display for CacheKeyTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeclarationId(declaration_id) => Display::fmt(declaration_id, f),
            Self::Download(url) => f.write_fmt(format_args!("Download({url})")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub execution_time: Duration,
    pub expires: Option<DateTime<Utc>>,
    pub etag: Option<String>,
    pub value: Value,
}

pub enum CacheStrategy {
    Always,
    Never,
}
