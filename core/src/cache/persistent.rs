use std::{io::ErrorKind, path::Path, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rain_lang::{
    afs::{
        area::FileArea,
        dir::Dir,
        entry::{FSEntry, FSEntryTrait as _},
        file::File,
    },
    runner::{
        cache::{CacheEntry, CacheKey},
        dep::Dep,
        internal::InternalFunction,
        value::{RainInteger, RainList, RainRecord, Value},
    },
};

use crate::config::Config;

pub const FORMAT_VERSION: u64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum PersistCacheError {
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("format missmatch")]
    FormatVersionMissmatch,
    #[error("does not exist")]
    DoesNotExist,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistCache {
    pub entries: Vec<(PersistCacheKey, PersistCacheEntry)>,
}

impl PersistCache {
    pub fn load(path: &Path) -> Result<Self, PersistCacheError> {
        let serialized = match std::fs::read(path) {
            Ok(serialized) => serialized,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                log::debug!("persistent cache did not exist");
                return Err(PersistCacheError::DoesNotExist);
            }
            Err(err) => return Err(err.into()),
        };
        let PersistCacheWrapper {
            format_version,
            inner,
        }: PersistCacheWrapper = serde_json::from_slice(&serialized)?;
        if format_version != FORMAT_VERSION {
            return Err(PersistCacheError::FormatVersionMissmatch);
        }
        Ok(serde_json::from_value(inner)?)
    }

    pub fn save(self, path: &Path) -> Result<(), PersistCacheError> {
        let Some(dir_path) = path.parent() else {
            return Err(PersistCacheError::DoesNotExist);
        };
        let p = PersistCacheWrapper {
            format_version: FORMAT_VERSION,
            inner: serde_json::to_value(self)?,
        };
        let serialized = serde_json::to_vec_pretty(&p)?;
        std::fs::create_dir_all(dir_path)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }

    pub fn persist(cache: &super::CacheCore, stats: &super::CacheStats) -> Self {
        let entries = cache
            .storage
            .iter()
            .filter_map(|(k, e)| {
                let Some(k) = PersistCacheKey::persist(k) else {
                    log::debug!("could not persist cache key {k:?}");
                    stats.persist_fails.inc();
                    return None;
                };
                let Some(e) = PersistCacheEntry::persist(e) else {
                    log::debug!("could not persist cache entry {e:?}");
                    stats.persist_fails.inc();
                    return None;
                };
                stats.persists.inc();
                Some((k, e))
            })
            .collect();
        Self { entries }
    }

    pub fn depersist(self, config: &Config, stats: &super::CacheStats) -> super::CacheCore {
        let mut lru = lru::LruCache::new(super::CACHE_SIZE);
        for (k, e) in self.entries {
            let Some(k) = k.depersist(config) else {
                log::warn!("could not depersist cache key for {e:?}");
                stats.depersist_fails.inc();
                continue;
            };
            let Some(e) = e.depersist(config) else {
                log::warn!("could not depersist cache entry");
                stats.depersist_fails.inc();
                continue;
            };
            stats.depersists.inc();
            lru.put(k, e);
        }
        super::CacheCore { storage: lru }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistCacheWrapper {
    pub format_version: u64,
    pub inner: serde_json::Value,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistCacheEntry {
    pub execution_time: Duration,
    pub expires: Option<DateTime<Utc>>,
    pub etag: Option<Vec<u8>>,
    pub deps: Vec<Dep>,
    pub value: PersistValue,
}

impl PersistCacheEntry {
    fn persist(entry: &CacheEntry) -> Option<Self> {
        if entry.deps.iter().any(|d| !d.is_inter_run_stable()) {
            // Don't cache because a dep is inter run unstable
            return None;
        }
        Some(Self {
            execution_time: entry.execution_time,
            expires: entry.expires,
            etag: entry.etag.clone(),
            deps: entry.deps.clone(),
            value: PersistValue::persist(&entry.value)?,
        })
    }

    fn depersist(self, config: &Config) -> Option<CacheEntry> {
        let value = self.value.depersist(config)?;
        Some(CacheEntry {
            execution_time: self.execution_time,
            expires: self.expires,
            etag: self.etag,
            deps: self.deps,
            value,
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PersistValue {
    Unit,
    Boolean(bool),
    Integer(RainInteger),
    String(String),
    FileArea(FileArea),
    File(FSEntry),
    Dir(FSEntry),
    Internal,
    InternalFunction(InternalFunction),
    List(Vec<PersistValue>),
    Record(IndexMap<String, PersistValue>),
}

impl PersistValue {
    fn persist(value: &Value) -> Option<Self> {
        match value {
            Value::Unit => Some(Self::Unit),
            Value::Boolean(b) => Some(Self::Boolean(*b)),
            Value::Integer(rain_integer) => Some(Self::Integer((**rain_integer).clone())),
            Value::String(s) => Some(Self::String((**s).clone())),
            // TODO: It is possible to persist these in the cache if we resolve the function/module id to a stable value and embed the File it was imported from
            Value::Function(_) | Value::Module(_) | Value::EscapeFile(_) => None,
            Value::FileArea(file_area) => Some(Self::FileArea((**file_area).clone())),
            Value::File(file) => Some(Self::File(file.inner().clone())),
            Value::Dir(dir) => Some(Self::Dir(dir.inner().clone())),
            Value::Internal => Some(Self::Internal),
            Value::InternalFunction(internal_function) => {
                Some(Self::InternalFunction(*internal_function))
            }
            Value::List(rain_list) => Some(Self::List(
                rain_list
                    .0
                    .iter()
                    .map(Self::persist)
                    .collect::<Option<_>>()?,
            )),
            Value::Record(rain_record) => Some(Self::Record(
                rain_record
                    .0
                    .iter()
                    .map(|(k, v)| Some((k.clone(), Self::persist(v)?)))
                    .collect::<Option<_>>()?,
            )),
        }
    }

    fn depersist(self, config: &Config) -> Option<Value> {
        match self {
            Self::Unit => Some(Value::Unit),
            Self::Boolean(b) => Some(Value::Boolean(b)),
            Self::Integer(rain_integer) => Some(Value::Integer(Arc::new(rain_integer))),
            Self::String(s) => Some(Value::String(Arc::new(s))),
            Self::FileArea(file_area) => Some(Value::FileArea(Arc::new(file_area))),
            Self::File(fsentry) => Some(Value::File(Arc::new(File::new_checked(config, fsentry)?))),
            Self::Dir(fsentry) => Some(Value::Dir(Arc::new(Dir::new_checked(config, fsentry)?))),
            Self::Internal => Some(Value::Internal),
            Self::InternalFunction(internal_function) => {
                Some(Value::InternalFunction(internal_function))
            }
            Self::List(vec) => Some(Value::List(Arc::new(RainList(
                vec.into_iter()
                    .map(|v| Self::depersist(v, config))
                    .collect::<Option<Vec<Value>>>()?,
            )))),
            Self::Record(index_map) => Some(Value::Record(Arc::new(RainRecord(
                index_map
                    .into_iter()
                    .map(|(k, v)| Some((k, Self::depersist(v, config)?)))
                    .collect::<Option<IndexMap<String, Value>>>()?,
            )))),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PersistCacheKey {
    InternalFunction {
        func: InternalFunction,
        args: Vec<PersistValue>,
    },
    Download {
        url: String,
    },
}

impl PersistCacheKey {
    fn persist(key: &CacheKey) -> Option<Self> {
        match key {
            // TODO: It is possible to persist declarations in the cache if we resolve the function/module id to a stable value and embed the File it was imported from
            // TODO: It is possible to persist prelude in the cache if we key it by the rain binary version
            CacheKey::Declaration { .. } | CacheKey::Prelude => None,
            CacheKey::InternalFunction { func, args } => Some(Self::InternalFunction {
                func: *func,
                args: args
                    .iter()
                    .map(PersistValue::persist)
                    .collect::<Option<_>>()?,
            }),
            CacheKey::Download { url } => Some(Self::Download { url: url.clone() }),
        }
    }

    fn depersist(self, config: &Config) -> Option<CacheKey> {
        match self {
            Self::InternalFunction { func, args } => Some(CacheKey::InternalFunction {
                func,
                args: args
                    .into_iter()
                    .map(|a| a.depersist(config))
                    .collect::<Option<Vec<Value>>>()?,
            }),
            Self::Download { url } => Some(CacheKey::Download { url }),
        }
    }
}
