use std::{collections::HashMap, io::ErrorKind, path::Path, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rain_lang::{
    afs::{
        File,
        generated::{
            area::GeneratedFSArea, dir::GeneratedDir, entry::GeneratedFSEntry, file::GeneratedFile,
        },
        local::{entry::LocalFSEntry, file::LocalFile},
    },
    ast::{Module, NodeId},
    hash::FileHash,
    ir::Rir,
    runner::{
        cache::{CacheEntry, CacheKey},
        dep_list::DepList,
        internal::InternalFunction,
        value::{ClosureCaptures, RainInteger, RainList, RainRecord, RainTypeId, Value},
    },
};
use tracing::{error, info, trace, warn};

use crate::config::Config;

pub const FORMAT_VERSION: u64 = 4;

#[derive(Debug, thiserror::Error)]
pub enum PersistCacheError {
    #[error("de: {0}")]
    De(#[from] ciborium::de::Error<std::io::Error>),
    #[error("ser: {0}")]
    Ser(#[from] ciborium::ser::Error<std::io::Error>),
    #[error("serde value: {0}")]
    SerdeValue(#[from] ciborium::value::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("format missmatch")]
    FormatVersionMissmatch,
    #[error("does not exist")]
    DoesNotExist,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PersistCache {
    pub entries: Vec<(PersistCacheKey, PersistCacheEntry)>,
}

impl PersistCache {
    pub fn load(path: &Path) -> Result<Self, PersistCacheError> {
        let serialized = match std::fs::read(path) {
            Ok(serialized) => serialized,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                info!("persistent cache did not exist");
                return Err(PersistCacheError::DoesNotExist);
            }
            Err(err) => return Err(err.into()),
        };
        let PersistCacheWrapper {
            format_version,
            inner,
        }: PersistCacheWrapper = ciborium::from_reader(&serialized[..])?;
        if format_version != FORMAT_VERSION {
            return Err(PersistCacheError::FormatVersionMissmatch);
        }
        Ok(inner.deserialized()?)
    }

    pub fn save(&self, path: &Path) -> Result<(), PersistCacheError> {
        let Some(dir_path) = path.parent() else {
            return Err(PersistCacheError::DoesNotExist);
        };
        let p = PersistCacheWrapper {
            format_version: FORMAT_VERSION,
            inner: ciborium::Value::serialized(&self)?,
        };
        std::fs::create_dir_all(dir_path)?;
        let f = std::fs::File::create(path)?;
        ciborium::into_writer(&p, f)?;
        Ok(())
    }

    pub fn persist(cache: &super::CacheCore, stats: &super::CacheStats, rir: &Rir) -> Self {
        let entries = cache
            .storage
            .iter()
            .filter_map(|(k, e)| {
                let Some(k) = PersistCacheKey::persist(k, rir) else {
                    trace!("could not persist cache key {k:?}");
                    stats.persist_fails.inc();
                    return None;
                };
                let Some(e) = PersistCacheEntry::persist(e, rir) else {
                    trace!("could not persist cache entry {e:?}");
                    stats.persist_fails.inc();
                    return None;
                };
                stats.persists.inc();
                Some((k, e))
            })
            .collect();
        Self { entries }
    }

    pub fn depersist(
        self,
        config: &Config,
        stats: &super::CacheStats,
        rir: &mut Rir,
    ) -> super::CacheCore {
        let mut lru = lru::LruCache::new(super::CACHE_SIZE);
        for (k, e) in self.entries {
            let Some(k) = k.depersist(config, rir) else {
                warn!("could not depersist cache key for {e:?}");
                stats.depersist_fails.inc();
                continue;
            };
            let Some(e) = e.depersist(config, rir) else {
                warn!("could not depersist cache entry");
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
    pub inner: ciborium::Value,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistCacheEntry {
    pub execution_time: Duration,
    pub expires: Option<DateTime<Utc>>,
    pub etag: Option<Vec<u8>>,
    pub deps: DepList,
    pub value: PersistValue,
}

impl PersistCacheEntry {
    fn persist(entry: &CacheEntry, rir: &Rir) -> Option<Self> {
        if entry.deps.iter().any(|d| !d.is_inter_run_stable()) {
            // Don't cache because a dep is inter run unstable
            return None;
        }
        Some(Self {
            execution_time: entry.execution_time,
            expires: entry.expires,
            etag: entry.etag.clone(),
            deps: entry.deps.clone(),
            value: PersistValue::persist(&entry.value, rir)?,
        })
    }

    fn depersist(self, config: &Config, rir: &mut Rir) -> Option<CacheEntry> {
        if self.deps.iter().any(|d| !d.is_inter_run_stable()) {
            // Don't depersist cache because a dep is inter run unstable
            return None;
        }
        let value = self.value.depersist(config, rir)?;
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
    GeneratedFSArea(GeneratedFSArea),
    GeneratedFile(GeneratedFSEntry),
    GeneratedDir(GeneratedFSEntry),
    Internal,
    InternalFunction(InternalFunction),
    List(Vec<Self>),
    Record(IndexMap<String, Self>),
    Module { file: PersistFile, src: String },
    Type(RainTypeId),
}

impl PersistValue {
    fn persist(value: &Value, rir: &Rir) -> Option<Self> {
        match value {
            Value::Unit => Some(Self::Unit),
            Value::Boolean(b) => Some(Self::Boolean(*b)),
            Value::Integer(rain_integer) => Some(Self::Integer((**rain_integer).clone())),
            Value::String(s) => Some(Self::String((**s).clone())),
            Value::Module(mid) => {
                let module = rir.get_module(*mid);
                Some(Self::Module {
                    file: PersistFile::persist(module.file.as_ref()?)?,
                    src: module.src.clone().into_owned(),
                })
            }
            Value::GeneratedFSArea(file_area) => Some(Self::GeneratedFSArea((**file_area).clone())),
            Value::GeneratedFile(file) => Some(Self::GeneratedFile(file.fsinner().clone())),
            Value::GeneratedDir(dir) => Some(Self::GeneratedDir(dir.fsinner().clone())),
            Value::Internal => Some(Self::Internal),
            Value::InternalFunction(internal_function) => {
                Some(Self::InternalFunction(*internal_function))
            }
            Value::List(rain_list) => Some(Self::List(
                rain_list
                    .0
                    .iter()
                    .map(|v| Self::persist(v, rir))
                    .collect::<Option<_>>()?,
            )),
            Value::Record(rain_record) => Some(Self::Record(
                rain_record
                    .0
                    .iter()
                    .map(|(k, v)| Some((k.clone(), Self::persist(v, rir)?)))
                    .collect::<Option<_>>()?,
            )),
            Value::Type(typ) => Some(Self::Type(*typ)),
            Value::EscapeFile(_)
            | Value::Closure(_)
            | Value::LocalFile(_)
            | Value::LocalDir(_)
            | Value::LocalFSArea(_)
            | Value::Unique(_) => None,
        }
    }

    fn depersist(self, config: &Config, rir: &mut Rir) -> Option<Value> {
        match self {
            Self::Unit => Some(Value::Unit),
            Self::Boolean(b) => Some(Value::Boolean(b)),
            Self::Integer(rain_integer) => Some(Value::Integer(Arc::new(rain_integer))),
            Self::String(s) => Some(Value::String(Arc::new(s))),
            Self::GeneratedFSArea(file_area) => Some(Value::GeneratedFSArea(Arc::new(file_area))),
            Self::GeneratedFile(fsentry) => Some(Value::GeneratedFile(Arc::new(
                GeneratedFile::new_checked(config, fsentry).ok()?,
            ))),
            Self::GeneratedDir(fsentry) => Some(Value::GeneratedDir(Arc::new(
                GeneratedDir::new_checked(config, fsentry)?,
            ))),
            Self::Internal => Some(Value::Internal),
            Self::InternalFunction(internal_function) => {
                Some(Value::InternalFunction(internal_function))
            }
            Self::List(vec) => Some(Value::List(Arc::new(RainList(
                vec.into_iter()
                    .map(|v| Self::depersist(v, config, rir))
                    .collect::<Option<Vec<Value>>>()?,
            )))),
            Self::Record(index_map) => Some(Value::Record(Arc::new(RainRecord(
                index_map
                    .into_iter()
                    .map(|(k, v)| Some((k, Self::depersist(v, config, rir)?)))
                    .collect::<Option<IndexMap<String, Value>>>()?,
            )))),
            Self::Module { file, src } => {
                let file = file.depersist(config)?;
                if let Some(mid) = rir.get_by_file(&file) {
                    return Some(Value::Module(mid));
                }
                let ast = Module::parse(&src);
                match rir.insert_module(Some(file), src, ast) {
                    Ok(mid) => Some(Value::Module(mid)),
                    Err(err) => {
                        error!("error loading cached module: {err:?}");
                        None
                    }
                }
            }
            Self::Type(typ) => Some(Value::Type(typ)),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PersistCacheKey {
    Declaration {
        module: PersistFile,
        name: String,
    },
    InternalFunction {
        func: InternalFunction,
        args: Vec<PersistValue>,
    },
    Download {
        url: String,
    },
    CallClosure {
        captures: HashMap<String, PersistValue>,
        module: PersistFile,
        node: NodeId,
        args: Vec<PersistValue>,
    },
}

impl PersistCacheKey {
    fn persist(key: &CacheKey, rir: &Rir) -> Option<Self> {
        match key {
            CacheKey::Declaration { module: file, name } => Some(Self::Declaration {
                module: PersistFile::persist(file)?,
                name: name.to_owned(),
            }),
            CacheKey::InternalFunction { func, args } => Some(Self::InternalFunction {
                func: *func,
                args: args
                    .iter()
                    .map(|v| PersistValue::persist(v, rir))
                    .collect::<Option<_>>()?,
            }),
            CacheKey::CallClosure {
                captures,
                module,
                node,
                args,
            } => Some(Self::CallClosure {
                captures: captures
                    .0
                    .iter()
                    .map(|(name, v)| Some((name.clone(), PersistValue::persist(v, rir)?)))
                    .collect::<Option<_>>()?,
                module: PersistFile::persist(module)?,
                node: *node,
                args: args
                    .iter()
                    .map(|v| PersistValue::persist(v, rir))
                    .collect::<Option<_>>()?,
            }),
            CacheKey::Download { url } => Some(Self::Download { url: url.clone() }),
            // TODO: It is possible to persist embed in the cache if we key it by the rain binary version
            CacheKey::Embed | CacheKey::Import { .. } => None,
        }
    }

    fn depersist(self, config: &Config, rir: &mut Rir) -> Option<CacheKey> {
        match self {
            Self::InternalFunction { func, args } => Some(CacheKey::InternalFunction {
                func,
                args: args
                    .into_iter()
                    .map(|a| a.depersist(config, rir))
                    .collect::<Option<Vec<Value>>>()?,
            }),
            Self::Download { url } => Some(CacheKey::Download { url }),
            Self::Declaration { module, name } => Some(CacheKey::Declaration {
                module: module.depersist(config)?,
                name,
            }),
            Self::CallClosure {
                captures,
                module,
                node,
                args,
            } => Some(CacheKey::CallClosure {
                captures: ClosureCaptures(Arc::new(
                    captures
                        .into_iter()
                        .map(|(name, v)| Some((name, v.depersist(config, rir)?)))
                        .collect::<Option<_>>()?,
                )),
                module: module.depersist(config)?,
                node,
                args: args
                    .into_iter()
                    .map(|a| a.depersist(config, rir))
                    .collect::<Option<Vec<Value>>>()?,
            }),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PersistFile {
    Generated(GeneratedFSEntry),
    Local(LocalFSEntry, FileHash),
}

impl PersistFile {
    #[expect(clippy::unnecessary_wraps)]
    fn persist(file: &File) -> Option<Self> {
        match file {
            File::Generated(file) => Some(Self::Generated(file.fsinner().clone())),
            File::Local(file) => Some(Self::Local(
                file.fsinner().clone(),
                file.file_hash().clone(),
            )),
        }
    }

    fn depersist(self, config: &Config) -> Option<File> {
        match self {
            Self::Generated(fsentry) => Some(File::Generated(
                GeneratedFile::new_checked(config, fsentry).ok()?,
            )),
            Self::Local(fsentry, hash) => {
                let file = LocalFile::new_checked(config, fsentry).ok()?;
                // If the hash doesn't match anymore we are not going to restore this cached file
                // This isn't really ideal but it is correct
                if file.file_hash() != &hash {
                    return None;
                }
                Some(File::Local(file))
            }
        }
    }
}
