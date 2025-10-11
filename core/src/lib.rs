pub mod cache;
pub mod config;
pub mod driver;

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub use rain_lang;

use driver::DriverImpl;
use rain_lang::{
    afs::entry::FSEntryTrait as _, driver::FSTrait as _, error::OwnedResolvedError,
    runner::value::Value,
};
use serde::{Deserialize, Serialize};

#[expect(clippy::result_unit_err, clippy::print_stderr)]
pub fn run_stderr(path: impl AsRef<Path>, declaration: &str) -> Result<Value, ()> {
    let driver = DriverImpl::new(config::Config::default());
    let cache = cache::Cache::default();
    run(path, declaration, &cache, &driver).map_err(|err| {
        eprintln!("{err}");
    })
}

pub fn run(
    path: impl AsRef<Path>,
    declaration: &str,
    cache: &cache::Cache,
    driver: &DriverImpl,
) -> Result<Value, CoreError> {
    let file = rain_lang::afs::file::File::new_local(path.as_ref())
        .map_err(|err| CoreError::Other(err.to_string()))?;
    let path = driver.resolve_fs_entry(file.inner());
    let src = std::fs::read_to_string(&path).map_err(|err| CoreError::Other(err.to_string()))?;
    let module = rain_lang::ast::parser::parse_module(&src);
    let mut ir = rain_lang::ir::Rir::new();
    let mid = ir
        .insert_module(Some(file), src, module)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(&ir).into_owned())))?;
    let main = ir
        .resolve_global_declaration(mid, declaration)
        .ok_or_else(|| CoreError::Other(String::from("declaration does not exist")))?;
    let mut runner = rain_lang::runner::Runner::new(&mut ir, cache, driver);
    let value = runner
        .evaluate_and_call(main, &[])
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(runner.ir).into_owned())))?;
    Ok(value)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CoreError {
    LangError(Box<OwnedResolvedError>),
    UnknownDeclaration(Vec<String>),
    Other(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LangError(owned_resolved_error) => owned_resolved_error.fmt(f),
            Self::UnknownDeclaration(suggestions) => f.write_fmt(format_args!(
                "unknown declaration, try one of {suggestions:?}"
            )),
            Self::Other(s) => std::fmt::Display::fmt(&s, f),
        }
    }
}

pub fn find_main_rain() -> Option<std::path::PathBuf> {
    let mut directory = std::env::current_dir().ok()?;
    loop {
        let p = directory.join("main.rain");
        if p.try_exists().is_ok_and(|b| b) {
            return Some(p);
        }
        if !directory.pop() {
            return None;
        }
    }
}

pub fn load_cache_or_default(config: &config::Config) -> (cache::Cache, rain_lang::ir::Rir) {
    let stats = cache::CacheStats::default();
    let mut ir = rain_lang::ir::Rir::new();
    match cache::persistent::PersistCache::load(&config.cache_json_path()) {
        Ok(p) => {
            let core = p.depersist(config, &stats, &mut ir);
            (
                cache::Cache {
                    core: Arc::new(Mutex::new(core)),
                    stats: Arc::new(stats),
                },
                ir,
            )
        }
        Err(err) => {
            log::info!("failed to load persist cache: {err}");
            (cache::Cache::default(), ir)
        }
    }
}
