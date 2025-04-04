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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistentCache {
    /// Map keyed by urls
    pub downloads: Vec<(String, PersistentCacheEntry)>,
}

impl PersistentCache {
    pub fn load(path: &Path) -> Result<Self, PersistentCacheError> {
        let serialized = match std::fs::read(path) {
            Ok(serialized) => serialized,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                log::debug!("persistent cache did not exist");
                return Err(PersistentCacheError::DoesNotExist);
            }
            Err(err) => return Err(err.into()),
        };
        let PersistentCacheWrapper {
            format_version,
            inner,
        }: PersistentCacheWrapper = serde_json::from_slice(&serialized)?;
        if format_version != FORMAT_VERSION {
            return Err(PersistentCacheError::FormatVersionMissmatch);
        }
        Ok(serde_json::from_value(inner)?)
    }

    pub fn save(self, path: &Path) -> Result<(), PersistentCacheError> {
        let p = PersistentCacheWrapper {
            format_version: FORMAT_VERSION,
            inner: serde_json::to_value(self)?,
        };
        let serialized = serde_json::to_vec_pretty(&p)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }

    pub fn from_cache(cache: &super::CacheCore) -> Self {
        let downloads = cache
            .storage
            .iter()
            .filter_map(|(k, e)| match k {
                CacheKey::InternalFunction { .. } | CacheKey::Declaration { .. } => None,
                CacheKey::Download { url } => Some((url.to_owned(), e.into())),
            })
            .collect();
        Self { downloads }
    }

    pub fn into_cache(self, config: &Config) -> super::CacheCore {
        let mut lru = lru::LruCache::new(super::CACHE_SIZE);
        for (url, e) in self.downloads {
            if let Some(e) = e.into_entry_if_present(config) {
                lru.put(CacheKey::Download { url }, e);
            }
        }
        super::CacheCore { storage: lru }
    }
}

pub const FORMAT_VERSION: u64 = 0;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistentCacheWrapper {
    pub format_version: u64,
    pub inner: serde_json::Value,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistentCacheEntry {
    pub execution_time: Duration,
    pub expires: Option<DateTime<Utc>>,
    pub etag: Option<String>,
    pub deps: Vec<Dep>,
    pub value: PersistentValue,
}

impl PersistentCacheEntry {
    fn into_entry_if_present(self, config: &Config) -> Option<CacheEntry> {
        let value = self.value.into_value_if_present(config)?;
        Some(CacheEntry {
            execution_time: self.execution_time,
            expires: self.expires,
            etag: self.etag,
            deps: self.deps,
            value,
        })
    }
}

impl From<&CacheEntry> for PersistentCacheEntry {
    fn from(entry: &CacheEntry) -> Self {
        Self {
            execution_time: entry.execution_time,
            expires: entry.expires,
            etag: entry.etag.clone(),
            deps: entry.deps.clone(),
            value: PersistentValue::from(&entry.value),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PersistentValue {
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
    List(Vec<PersistentValue>),
    Record(IndexMap<String, PersistentValue>),
}

impl PersistentValue {
    fn into_value_if_present(self, config: &Config) -> Option<Value> {
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
                    .map(|v| Self::into_value_if_present(v, config))
                    .collect::<Option<Vec<Value>>>()?,
            )))),
            Self::Record(index_map) => Some(Value::Record(Arc::new(RainRecord(
                index_map
                    .into_iter()
                    .map(|(k, v)| Some((k, Self::into_value_if_present(v, config)?)))
                    .collect::<Option<IndexMap<String, Value>>>()?,
            )))),
        }
    }
}

impl From<&Value> for PersistentValue {
    fn from(value: &Value) -> Self {
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
            Value::List(rain_list) => {
                Self::List(rain_list.0.iter().map(std::convert::Into::into).collect())
            }
            Value::Record(rain_record) => Self::Record(
                rain_record
                    .0
                    .iter()
                    .map(|(k, v)| (k.clone(), v.into()))
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PersistentCacheError {
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("format missmatch")]
    FormatVersionMissmatch,
    #[error("does not exist")]
    DoesNotExist,
}
