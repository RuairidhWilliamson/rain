pub mod persistent;

use std::{
    collections::HashSet,
    num::NonZeroUsize,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use lru::LruCache;
use poison_panic::MutexExt as _;
use rain_lang::{
    afs::{area::FileAreaRef, generated::area::GeneratedFSArea},
    driver::FSTrait,
    runner::{
        LocalFileHashCache,
        cache::{CacheEntry, CacheGuardTrait, CacheKey, CacheTrait},
        dep_list::DepList,
        value::{RainTypeId, Value},
    },
};

const CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(10240).expect("cache size must be non zero");

#[derive(Default, Clone)]
pub struct Cache {
    /// Minimum execution time to be stored in the cache
    pub execution_time_thresold: Duration,
    pub core: Arc<Mutex<CacheCore>>,
    pub stats: Arc<CacheStats>,
    pub verification: bool,
}

impl Cache {
    pub fn new(core: CacheCore) -> Self {
        Self {
            execution_time_thresold: Duration::from_millis(1),
            core: Arc::new(Mutex::new(core)),
            stats: Arc::default(),
            verification: false,
        }
    }

    pub fn len(&self) -> usize {
        self.core.plock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.core.plock().is_empty()
    }
}

pub struct CacheGuard {
    cache: Cache,
    start: Instant,
    key: Option<CacheKey>,
    existing_entry: Option<CacheEntry>,
    verification: bool,
}

impl CacheGuard {
    fn verify(&self, value: &Value) {
        let Some(key) = &self.key else {
            return;
        };
        let Some(existing_entry) = &self.existing_entry else {
            unreachable!();
        };
        // Skip some things for now
        if [
            RainTypeId::Module,
            RainTypeId::GeneratedFile,
            RainTypeId::GeneratedFSArea,
            RainTypeId::GeneratedDir,
        ]
        .contains(&value.rain_type_id())
        {
            return;
        }
        if value != &existing_entry.value {
            log::error!(
                "cache violation {key:?}\nexisting cache entry = {existing_entry:?}\nactual value = {value:?}"
            );
        }
    }
}

impl CacheGuardTrait for CacheGuard {
    fn check(&mut self) -> Option<(Value, DepList)> {
        if self.key.is_some()
            && !self.verification
            && let Some(existing_entry) = self.existing_entry.take()
        {
            Some((existing_entry.value, existing_entry.deps))
        } else {
            None
        }
    }

    fn put(self, deps: DepList, value: Value) {
        if self.key.is_none() {
            return;
        }
        if self.verification && self.existing_entry.is_some() {
            self.verify(&value);
            return;
        }
        let Some(key) = self.key else {
            return;
        };
        self.cache.put(
            key,
            CacheEntry {
                execution_time: self.start.elapsed(),
                expires: None,
                etag: None,
                deps,
                value,
            },
        );
    }

    fn put_if_slow(self, deps: DepList, value: Value) {
        if self.key.is_none() {
            return;
        }
        if self.verification && self.existing_entry.is_some() {
            self.verify(&value);
            return;
        }
        let Some(key) = self.key else {
            return;
        };
        self.cache.put_if_slow(
            key,
            CacheEntry {
                execution_time: self.start.elapsed(),
                expires: None,
                etag: None,
                deps,
                value,
            },
        );
    }
}

impl CacheTrait for Cache {
    type CacheGuard = CacheGuard;

    fn get(
        &self,
        key: &CacheKey,
        fs: &impl FSTrait,
        lfhc: &mut LocalFileHashCache,
    ) -> Option<CacheEntry> {
        let mut guard = self.core.plock();
        let res = guard.storage.get(key).cloned();
        if let Some(entry) = &res {
            for d in entry.deps.iter() {
                if !d.is_valid(fs, lfhc) {
                    log::trace!("cache get miss because dep is not valid {key:?} {d:?}");
                    return None;
                }
            }
            self.stats.hits.inc();
            log::trace!("cache get hit {key:?} {:?}", entry.deps);
        } else {
            self.stats.misses.inc();
            log::debug!("cache get miss {key:?}");
        }
        res
    }

    fn put(&self, key: CacheKey, entry: CacheEntry) {
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
        if entry.execution_time < self.execution_time_thresold {
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

    fn guard(
        &self,
        key: Option<CacheKey>,
        fs: &impl FSTrait,
        lfhc: &mut LocalFileHashCache,
    ) -> Self::CacheGuard {
        CacheGuard {
            cache: self.clone(),
            existing_entry: key.as_ref().and_then(|key| self.get(key, fs, lfhc)),
            key,
            verification: self.verification,
            start: Instant::now(),
        }
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

    pub fn get_all_generated_areas(&self) -> HashSet<&GeneratedFSArea> {
        let mut out = HashSet::new();
        for (_, entry) in &self.storage {
            for area in entry.value.find_areas() {
                if let FileAreaRef::Generated(generated_file_area) = area {
                    out.insert(generated_file_area);
                }
            }
        }
        out
    }

    pub fn prune_generated_areas(
        &self,
        config: &crate::config::Config,
    ) -> std::io::Result<PruneStats> {
        let mut stats = PruneStats { size: 0, errors: 0 };
        log::info!("Pruning");
        let connected = self.get_all_generated_areas();
        for entry in std::fs::read_dir(&config.base_generated_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(id) = uuid::Uuid::parse_str(&name) else {
                continue;
            };
            let area = GeneratedFSArea { id };
            if connected.contains(&area) {
                log::info!("Not Pruning {area:?}");
                continue;
            }
            log::info!("Pruning {area:?}");
            match remove_recursive(&entry.path()) {
                Ok(s) => {
                    stats.size += s;
                }
                Err(err) => {
                    log::error!("Failed to prune {area:?} because {err}");
                    stats.errors += 1;
                }
            }
        }
        log::info!("Prune complete");
        Ok(stats)
    }
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: Counter,
    pub misses: Counter,
    pub puts: Counter,
    pub put_fails: Counter,
    pub depersists: Counter,
    pub depersist_fails: Counter,
    pub persists: Counter,
    pub persist_fails: Counter,
}

#[derive(Default)]
pub struct Counter(AtomicUsize);

impl Counter {
    pub fn get(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    pub fn inc(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

impl std::fmt::Debug for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

fn remove_recursive(path: &Path) -> std::io::Result<u64> {
    let metadata = std::fs::symlink_metadata(path)?;
    let filetype = metadata.file_type();
    if filetype.is_symlink() {
        std::fs::remove_file(path)?;
        return Ok(metadata.len());
    }
    remove_dir_all_recursive(path)
}

fn remove_dir_all_recursive(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    let stat = std::fs::symlink_metadata(path)
        .inspect_err(|err| log::error!("metadata {path:?} error: {err}"))?;
    if stat.is_symlink() {
        std::fs::remove_file(path)?;
        return Ok(0);
    }
    ensure_writable(path, &stat)
        .inspect_err(|err| log::error!("ensure writable {path:?} error: {err}"))?;
    for child in
        std::fs::read_dir(path).inspect_err(|err| log::error!("read dir {path:?} error: {err}"))?
    {
        let child = child?;
        let ftype = child.file_type()?;
        let child_path = child.path();
        if ftype.is_dir() && !ftype.is_symlink() {
            size += remove_dir_all_recursive(&child_path)?;
        } else {
            let metadata = child.metadata()?;
            size += metadata.len();
            std::fs::remove_file(&child_path)?;
        }
    }
    std::fs::remove_dir(path)?;
    Ok(size)
}

#[cfg(not(target_family = "unix"))]
fn ensure_writable(_path: &Path, _stat: &std::fs::Metadata) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_family = "unix")]
fn ensure_writable(path: &Path, stat: &std::fs::Metadata) -> std::io::Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    assert!(!stat.is_symlink());
    let mode = stat.mode();
    if mode & 0o700 != 0o700 {
        let mut perm = stat.permissions();
        perm.set_mode(mode | 0o700);
        std::fs::set_permissions(path, perm)?;
    }
    Ok(())
}

pub struct PruneStats {
    pub size: u64,
    pub errors: u32,
}
