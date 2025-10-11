use std::sync::{Arc, Mutex};

use log::{error, info};
use poison_panic::MutexExt as _;
use rain_core::{
    cache::{Cache, CacheStats, persistent::PersistCache},
    config::Config,
    driver::DriverImpl,
};
use rain_lang::{
    afs::{area::FileArea, entry::FSEntry, file::File, path::SealedFilePath},
    driver::DriverTrait as _,
};

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

    #[expect(clippy::unwrap_used)]
    pub fn run(&self, driver: &DriverImpl, area: FileArea) -> RunComplete {
        let root_entry = FSEntry::new(area, SealedFilePath::new("/main.rain").unwrap());
        info!("Root entry {root_entry}");
        let root = File::new_checked(driver, root_entry).unwrap();
        let src = driver.read_file(&root).unwrap();
        let module = rain_lang::ast::parser::parse_module(&src);
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
        let declaration = "ci";
        let main = ir.resolve_global_declaration(mid, declaration).unwrap();
        let mut persistent_cache = self.persistent_cache.plock();
        let cache_core = persistent_cache
            .take()
            .map(|c| c.depersist(&self.config, &self.cache_stats, &mut ir))
            .unwrap_or_default();
        let cache = Cache {
            core: Arc::new(Mutex::new(cache_core)),
            stats: Arc::clone(&self.cache_stats),
        };
        let mut runner = rain_lang::runner::Runner::new(&mut ir, &cache, driver);
        runner.seal = self.seal;
        info!("Running");
        let res = runner.evaluate_and_call(main, &[]);
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
                let err = err.resolve_ir(&ir);
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
