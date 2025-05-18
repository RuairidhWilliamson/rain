use std::sync::Arc;

use poison_panic::MutexExt as _;
use rain_core::{
    cache::{Cache, persistent::PersistCache},
    config::Config,
};
use rain_lang::{
    afs::{dir::Dir, entry::FSEntry, file::File, path::FilePath},
    driver::{DriverTrait as _, FSTrait as _},
};

#[derive(Clone)]
pub struct Runner {
    config: Arc<Config>,
    cache: Cache,
}

impl Runner {
    pub fn new() -> Self {
        let config = Arc::new(rain_core::config::Config::new());
        let cache = rain_core::load_cache_or_default(&config);
        Self { config, cache }
    }

    #[expect(clippy::unwrap_used, clippy::cognitive_complexity)]
    pub fn run(&self, download: &[u8], download_dir_name: &str) -> RunComplete {
        let declaration = "ci";
        let mut ir = rain_lang::ir::Rir::new();
        let driver = rain_core::driver::DriverImpl::new(self.config.as_ref().clone());
        let download_area = driver.create_area(&[]).unwrap();
        let download_entry = FSEntry::new(download_area, FilePath::new("/download").unwrap());
        std::fs::write(driver.resolve_fs_entry(&download_entry), download).unwrap();
        let download = File::new_checked(&driver, download_entry).unwrap();
        let area = driver.extract_tar_gz(&download).unwrap();
        let download_dir_entry = FSEntry::new(area, FilePath::new(download_dir_name).unwrap());
        let root = Dir::new_checked(&driver, download_dir_entry).unwrap();
        let area = driver.create_area(&[&root]).unwrap();
        let root_entry = FSEntry::new(area, FilePath::new("/root.rain").unwrap());
        tracing::info!("Root entry {root_entry}");
        let root = File::new_checked(&driver, root_entry).unwrap();
        let src = driver.read_file(&root).unwrap();
        let module = rain_lang::ast::parser::parse_module(&src);
        let mid = ir.insert_module(Some(root), src, module).unwrap();
        let main = ir.resolve_global_declaration(mid, declaration).unwrap();
        let mut runner = rain_lang::runner::Runner::new(&mut ir, &self.cache, &driver);
        tracing::info!("Running");
        let res = runner.evaluate_and_call(main, &[]);
        let persistent_cache = PersistCache::persist(&self.cache.0.plock());
        persistent_cache
            .save(&driver.config.cache_json_path())
            .unwrap();
        match res {
            Ok(value) => {
                tracing::info!("Value {value}");
                RunComplete {
                    success: true,
                    output: format!("{value}"),
                }
            }
            Err(err) => {
                tracing::error!("{err:?}");
                let err = err.resolve_ir(&ir);
                tracing::error!("\n{err}");
                RunComplete {
                    success: false,
                    output: format!("{err}"),
                }
            }
        }
    }
}

pub struct RunComplete {
    pub success: bool,
    pub output: String,
}
