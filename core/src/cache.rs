pub mod persistent;

use std::{
    collections::HashSet,
    num::NonZeroUsize,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use lru::LruCache;
use poison_panic::MutexExt as _;
use rain_lang::{
    afs::area::{FileArea, GeneratedFileArea},
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
            let area = GeneratedFileArea { id };
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
        std::fs::remove_file(&path)?;
        return Ok(0);
    }
    ensure_writable(path, stat)
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
fn ensure_writable(_path: &Path, _stat: std::fs::Metadata) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_family = "unix")]
fn ensure_writable(path: &Path, stat: std::fs::Metadata) -> std::io::Result<()> {
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
