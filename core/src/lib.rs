use config::Config;
use driver::DriverImpl;
use rain_lang::{
    driver::DriverTrait as _,
    error::OwnedResolvedError,
    runner::cache::{Cache, CACHE_SIZE},
};
use serde::{Deserialize, Serialize};

pub mod config;
pub mod driver;
pub mod exe;
pub mod ipc;
pub mod remote;

#[expect(clippy::result_unit_err)]
pub fn run_log(
    path: impl AsRef<std::path::Path>,
    declaration: &str,
) -> Result<rain_lang::runner::value::Value, ()> {
    let driver = DriverImpl::new(Config::default());
    let mut cache = Cache::new(CACHE_SIZE);
    run(path, declaration, &mut cache, &driver).map_err(|err| {
        log::error!("{err}");
    })
}

pub fn run(
    path: impl AsRef<std::path::Path>,
    declaration: &str,
    cache: &mut Cache,
    file_system: &DriverImpl,
) -> Result<rain_lang::runner::value::Value, CoreError> {
    let file = rain_lang::afs::file::File::new_local(path.as_ref())
        .map_err(|err| CoreError::Other(err.to_string()))?;
    let path = file_system.resolve_file(&file);
    let src = std::fs::read_to_string(&path).map_err(|err| CoreError::Other(err.to_string()))?;
    let module = rain_lang::ast::parser::parse_module(&src);
    let mut ir = rain_lang::ir::Rir::new();
    let mid = ir
        .insert_module(file, src, module)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(&ir).into_owned())))?;
    let main = ir
        .resolve_global_declaration(mid, declaration)
        .ok_or_else(|| CoreError::Other(String::from("declaration does not exist")))?;
    let mut runner = rain_lang::runner::Runner::new(ir, cache, file_system);
    let value = runner
        .evaluate_and_call(main)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(&runner.ir).into_owned())))?;
    Ok(value)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CoreError {
    LangError(Box<OwnedResolvedError>),
    Other(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LangError(owned_resolved_error) => owned_resolved_error.fmt(f),
            Self::Other(s) => s.fmt(f),
        }
    }
}

pub fn find_root_rain() -> Option<std::path::PathBuf> {
    let mut directory = std::env::current_dir().ok()?;
    loop {
        let p = directory.join("root.rain");
        if p.try_exists().is_ok_and(|b| b) {
            return Some(p);
        }
        if !directory.pop() {
            return None;
        }
    }
}
