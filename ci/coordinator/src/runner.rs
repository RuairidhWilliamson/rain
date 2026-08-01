use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use alias::Alias as _;
use poison_panic::MutexExt as _;
use rain_core::{
    cache::{Cache, CacheStats, persistent::PersistCache},
    config::Config,
    driver::DriverImpl,
};
use rain_lang::{
    afs::{
        File,
        area::FSArea,
        entry::FSEntry,
        generated::{area::GitDescribe, dir::GeneratedDir},
        path::SealedFilePath,
    },
    ast::Module,
    cancellation::Cancellation,
    driver::{CreateAreaOptions, DriverTrait as _},
    runner::dep_list::DepList,
};
use tracing::{error, info};

#[derive(Clone)]
pub struct Runner {
    config: Arc<Config>,
    persistent_cache: Arc<Mutex<Option<PersistCache>>>,
    cache_stats: Arc<CacheStats>,
    seal: bool,
}

impl Runner {
    pub fn new(seal: bool) -> Self {
        let config = Arc::new(rain_core::config::Config::new());
        let persistent_cache = Arc::new(Mutex::new(None));
        Self {
            config,
            persistent_cache,
            cache_stats: Default::default(),
            seal,
        }
    }

    fn create_driver_for_run(secrets: HashMap<String, String>) -> DriverImpl<'static> {
        let mut driver = DriverImpl::new(rain_core::config::Config::new());
        driver.secrets = rain_core::driver::Secrets::Set(secrets);
        driver
    }

    #[expect(clippy::unwrap_used)]
    fn create_area_for_run(
        root: &GeneratedDir,
        driver: &DriverImpl,
        sha: String,
    ) -> rain_lang::afs::generated::area::GeneratedFSArea {
        let mut area = driver
            .create_overlay_area(
                std::iter::once(root.fsinner().into()),
                &CreateAreaOptions {
                    flatten_input_dirs: true,
                    ..Default::default()
                },
            )
            .unwrap();
        area.git_describe = Some(GitDescribe {
            commit: sha,
            dirty: false,
        });
        area
    }

    pub fn run(
        &self,
        root: &GeneratedDir,
        RunOptions {
            secrets,
            sha,
            target,
            cancel,
        }: RunOptions,
    ) -> RunComplete {
        let driver = Self::create_driver_for_run(secrets);
        let area = Self::create_area_for_run(root, &driver, sha);
        self.run_inner(&driver, FSArea::Generated(area), &target, cancel)
    }

    #[expect(clippy::unwrap_used)]
    fn run_inner(
        &self,
        driver: &DriverImpl,
        area: FSArea,
        target: &str,
        cancel: Cancellation,
    ) -> RunComplete {
        let root_entry = FSEntry::new(area, SealedFilePath::new("/main.rain").unwrap());
        info!("Root entry {root_entry}");
        let root = File::new_checked(driver, root_entry).unwrap();
        let src = driver.read_file(&root).unwrap();
        let module = Module::parse(&src);
        let mut ir = rain_lang::ir::Rir::new();
        let mid = match ir.insert_module(Some(root), src, module) {
            Ok(mid) => mid,
            Err(err) => {
                let err = err.resolve_ir(&ir);
                error!("\n{err}");
                return RunComplete {
                    success: false,
                    output: format!("{err}"),
                };
            }
        };
        let mut persistent_cache = self.persistent_cache.plock();
        let cache_core = persistent_cache
            .take()
            .map(|c| c.depersist(&self.config, &self.cache_stats, &mut ir))
            .unwrap_or_default();
        let cache = Cache {
            core: Arc::new(Mutex::new(cache_core)),
            stats: self.cache_stats.alias(),
            ..Default::default()
        };
        let mut runner = rain_lang::runner::Runner::new(&mut ir, &cache, driver, cancel);
        runner.seal = self.seal;
        info!("Running");
        let mut deps = DepList::new();
        let res = rain_core::evaluate_and_call_chain(&mut runner, mid, &mut deps, target, &[]);
        let new_persistent_cache =
            PersistCache::persist(&cache.core.plock(), &self.cache_stats, &ir);
        *persistent_cache = Some(new_persistent_cache);
        let prints = strip_ansi_escapes::strip_str(driver.prints.plock().join("\n"));
        match res {
            Ok(value) => {
                info!("Value {value}");
                RunComplete {
                    success: true,
                    output: format!("{prints}\n--\n{value:#}"),
                }
            }
            Err(err) => {
                error!("{err:?}");
                error!("\n{err}");
                RunComplete {
                    success: false,
                    output: format!("{prints}\n--\n{err}"),
                }
            }
        }
    }

    pub fn prune(&self) {
        let mut persistent_cache = self.persistent_cache.plock();
        let Some(pcache) = persistent_cache.take() else {
            return;
        };
        let mut ir = rain_lang::ir::Rir::new();
        let cache = pcache.depersist(&self.config, &self.cache_stats, &mut ir);
        if let Err(err) = cache.prune_generated_areas(&self.config) {
            error!("prune error: {err:#}");
        }
        *persistent_cache = Some(PersistCache::persist(&cache, &self.cache_stats, &ir));
    }
}

pub struct RunComplete {
    pub success: bool,
    pub output: String,
}

pub struct RunOptions {
    pub secrets: HashMap<String, String>,
    pub sha: String,
    pub target: String,
    pub cancel: Cancellation,
}
