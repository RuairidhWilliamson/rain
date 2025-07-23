use std::sync::Arc;

use log::{error, info};
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
    seal: bool,
}

impl Runner {
    pub fn new(seal: bool) -> Self {
        let config = Arc::new(rain_core::config::Config::new());
        let cache = rain_core::load_cache_or_default(&config);
        Self {
            config,
            cache,
            seal,
        }
    }

    #[expect(clippy::unwrap_used)]
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
        let root_entry = FSEntry::new(area, FilePath::new("/main.rain").unwrap());
        info!("Root entry {root_entry}");
        let root = File::new_checked(&driver, root_entry).unwrap();
        let src = driver.read_file(&root).unwrap();
        let module = rain_lang::ast::parser::parse_module(&src);
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
        let main = ir.resolve_global_declaration(mid, declaration).unwrap();
        let mut runner = rain_lang::runner::Runner::new(&mut ir, &self.cache, &driver);
        runner.seal = self.seal;
        info!("Running");
        let res = runner.evaluate_and_call(main, &[]);
        let persistent_cache = PersistCache::persist(&self.cache.core.plock(), &self.cache.stats);
        if let Err(err) = persistent_cache.save(&driver.config.cache_json_path()) {
            error!("save persist cache failed: {err:#}");
        }
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
}

pub struct RunComplete {
    pub success: bool,
    pub output: String,
}
