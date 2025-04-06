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
    ir::{DeclarationId, ModuleId},
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
        let p = PersistCacheWrapper {
            format_version: FORMAT_VERSION,
            inner: serde_json::to_value(self)?,
        };
        let serialized = serde_json::to_vec_pretty(&p)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }

    pub fn persist(cache: &super::CacheCore) -> Self {
        let entries = cache
            .storage
            .iter()
            .map(|(k, e)| (PersistCacheKey::persist(k), PersistCacheEntry::persist(e)))
            .collect();
        Self { entries }
    }

    pub fn depersist(self, config: &Config) -> super::CacheCore {
        let mut lru = lru::LruCache::new(super::CACHE_SIZE);
        for (k, e) in self.entries {
            if let Some(e) = e.depersist(config) {
                if let Some(k) = k.depersist(config) {
                    lru.put(k, e);
                }
            }
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
    pub etag: Option<String>,
    pub deps: Vec<Dep>,
    pub value: PersistValue,
}

impl PersistCacheEntry {
    fn persist(entry: &CacheEntry) -> Self {
        Self {
            execution_time: entry.execution_time,
            expires: entry.expires,
            etag: entry.etag.clone(),
            deps: entry.deps.clone(),
            value: PersistValue::persist(&entry.value),
        }
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
    Function(DeclarationId),
    Module(ModuleId),
    FileArea(FileArea),
    File(FSEntry),
    Dir(FSEntry),
    Internal,
    InternalFunction(InternalFunction),
    List(Vec<PersistValue>),
    Record(IndexMap<String, PersistValue>),
}

impl PersistValue {
    fn persist(value: &Value) -> Self {
        match value {
            Value::Unit => Self::Unit,
            Value::Boolean(b) => Self::Boolean(*b),
            Value::Integer(rain_integer) => Self::Integer((**rain_integer).clone()),
            Value::String(s) => Self::String((**s).clone()),
            Value::Function(declaration_id) => Self::Function(*declaration_id),
            Value::Module(module_id) => Self::Module(*module_id),
            Value::FileArea(file_area) => Self::FileArea((**file_area).clone()),
            Value::File(file) => Self::File(file.inner().clone()),
            Value::Dir(dir) => Self::Dir(dir.inner().clone()),
            Value::Internal => Self::Internal,
            Value::InternalFunction(internal_function) => {
                Self::InternalFunction(*internal_function)
            }
            Value::List(rain_list) => Self::List(rain_list.0.iter().map(Self::persist).collect()),
            Value::Record(rain_record) => Self::Record(
                rain_record
                    .0
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::persist(v)))
                    .collect(),
            ),
        }
    }

    fn depersist(self, config: &Config) -> Option<Value> {
        match self {
            Self::Unit => Some(Value::Unit),
            Self::Boolean(b) => Some(Value::Boolean(b)),
            Self::Integer(rain_integer) => Some(Value::Integer(Arc::new(rain_integer))),
            Self::String(s) => Some(Value::String(Arc::new(s))),
            Self::Function(declaration_id) => Some(Value::Function(declaration_id)),
            Self::Module(module_id) => Some(Value::Module(module_id)),
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
    Declaration {
        declaration: DeclarationId,
        args: Vec<PersistValue>,
    },
    InternalFunction {
        func: InternalFunction,
        args: Vec<PersistValue>,
    },
    Download {
        url: String,
    },
}

impl PersistCacheKey {
    fn persist(key: &CacheKey) -> Self {
        match key {
            CacheKey::Declaration { declaration, args } => Self::Declaration {
                declaration: *declaration,
                args: args.iter().map(PersistValue::persist).collect(),
            },
            CacheKey::InternalFunction { func, args } => Self::InternalFunction {
                func: *func,
                args: args.iter().map(PersistValue::persist).collect(),
            },
            CacheKey::Download { url } => Self::Download { url: url.clone() },
        }
    }

    fn depersist(self, config: &Config) -> Option<CacheKey> {
        Some(match self {
            Self::Declaration { declaration, args } => CacheKey::Declaration {
                declaration,
                args: args
                    .into_iter()
                    .map(|a| a.depersist(config))
                    .collect::<Option<Vec<Value>>>()?,
            },
            Self::InternalFunction { func, args } => CacheKey::InternalFunction {
                func,
                args: args
                    .into_iter()
                    .map(|a| a.depersist(config))
                    .collect::<Option<Vec<Value>>>()?,
            },
            Self::Download { url } => CacheKey::Download { url },
        })
    }
}
