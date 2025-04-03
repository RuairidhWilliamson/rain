use std::{io::ErrorKind, path::Path, time::Duration};

use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rain_lang::runner::{
    cache::{CacheEntry, CacheKey},
    dep::Dep,
    value::Value,
};

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
}

impl From<&super::CacheCore> for PersistentCache {
    fn from(cache: &super::CacheCore) -> Self {
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
}

impl From<PersistentCache> for super::CacheCore {
    fn from(p: PersistentCache) -> Self {
        let mut lru = lru::LruCache::new(super::CACHE_SIZE);
        for (url, e) in p.downloads {
            if let Some(e) = e.into_entry_if_present() {
                lru.put(CacheKey::Download { url }, e);
            }
        }
        Self { storage: lru }
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
    fn into_entry_if_present(self) -> Option<CacheEntry> {
        let value = self.value.into_value_if_present()?;
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
    Record(IndexMap<String, PersistentValue>),
}

impl PersistentValue {
    fn into_value_if_present(self) -> Option<Value> {
        todo!()
    }
}

impl From<&Value> for PersistentValue {
    fn from(value: &Value) -> Self {
        todo!()
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
