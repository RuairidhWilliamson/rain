pub mod cache;
pub mod config;
pub mod driver;

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use alias::Alias as _;
pub use rain_lang;

use driver::DriverImpl;
use rain_lang::{
    afs::{File, local::file::LocalFile},
    ast::Module,
    driver::FSTrait as _,
    error::OwnedResolvedError,
    ir::ModuleId,
    runner::{cx::Cx, dep_list::DepList, value::Value},
};
use serde::{Deserialize, Serialize};

type Runner<'a> = rain_lang::runner::Runner<'a, DriverImpl<'a>, cache::Cache>;

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
    target: &str,
    cache: &cache::Cache,
    driver: &DriverImpl,
) -> Result<Value, CoreError> {
    let file = LocalFile::new_local(driver, path.as_ref())
        .map_err(|err| CoreError::Other(err.to_string()))?;
    let path = driver.resolve_fs_entry(file.fsinner().into());
    let src = std::fs::read_to_string(&path).map_err(|err| CoreError::Other(err.to_string()))?;
    let module = Module::parse(&src);
    let mut ir = rain_lang::ir::Rir::new();
    let mid = ir
        .insert_module(Some(File::Local(file)), src, module)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(&ir).into_owned())))?;
    let mut runner = rain_lang::runner::Runner::new(&mut ir, cache, driver);
    let mut deps = DepList::new();
    evaluate_and_call_chain(&mut runner, mid, &mut deps, target, &[])
}

pub fn new_runner<'a>(
    ir: &'a mut rain_lang::ir::Rir,
    cache: &'a cache::Cache,
    driver: &'a DriverImpl<'a>,
) -> Runner<'a> {
    rain_lang::runner::Runner::new(ir, cache, driver)
}

pub fn insert_local_module(
    runner: &mut Runner,
    path: impl AsRef<Path>,
) -> Result<ModuleId, CoreError> {
    let file = LocalFile::new_local(runner.driver, path.as_ref())
        .map_err(|err| CoreError::Other(err.to_string()))?;
    let path = runner.driver.resolve_fs_entry(file.fsinner().into());
    let src = std::fs::read_to_string(&path).map_err(|err| CoreError::Other(err.to_string()))?;
    let module = Module::parse(&src);
    let mid = runner
        .ir
        .insert_module(Some(File::Local(file)), src, module)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(runner.ir).into_owned())))?;
    Ok(mid)
}

pub fn evaluate_and_call_chain(
    runner: &mut Runner,
    mut mid: ModuleId,
    deps: &mut DepList,
    targets: &str,
    args: &[String],
) -> Result<Value, CoreError> {
    let initial_module = runner.ir.get_module(mid).alias();
    let mut cx = Cx::new(&initial_module, 0, HashMap::new(), Vec::new());
    runner
        .check_module(&mut cx, initial_module.id)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(runner.ir).into_owned())))?;
    let mut v = None;
    let mut mid_nid = None;
    let target_chain: Vec<_> = targets.split('.').collect();
    for (i, &target) in target_chain.iter().enumerate() {
        let Some(declaration) = runner.ir.resolve_global_declaration(mid, target) else {
            let suggestions: Vec<String> = runner
                .ir
                .suggest_declarations(mid)
                .into_iter()
                .map(std::borrow::ToOwned::to_owned)
                .collect();
            let mut prefix = String::new();
            if i > 0 {
                prefix = target_chain[..i].join(".") + ".";
            }
            return Err(CoreError::UnknownDeclaration {
                prefix,
                unknown: target.to_owned(),
                suggestions,
            });
        };
        let m = runner.ir.get_module(mid).alias();
        let nid = m.get_declaration(declaration.local_id()).assignment.expr;
        mid_nid = Some((mid, nid));

        let mut initial_cx = Cx::new(&m, 0, HashMap::new(), Vec::new());

        v = Some(
            runner
                .evaluate_declaration(&mut initial_cx, declaration)
                .map_err(|err| {
                    CoreError::LangError(Box::new(err.resolve_ir(runner.ir).into_owned()))
                })?,
        );
        deps.merge(initial_cx.deps);
        match v {
            Some(Value::Module(deeper_mid)) => {
                mid = deeper_mid;
            }
            _ => {
                break;
            }
        }
    }
    match v {
        Some(Value::Closure(closure)) => {
            let args: Vec<Value> = args
                .iter()
                .map(|v| Value::String(Arc::new(v.clone())))
                .collect();
            let Some((mid, nid)) = mid_nid else {
                unreachable!()
            };
            let result = runner
                .call_closure(
                    &mut cx,
                    nid,
                    runner.ir.get_module(mid).span(nid),
                    &closure,
                    args,
                )
                .map_err(|err| {
                    CoreError::LangError(Box::new(err.resolve_ir(runner.ir).into_owned()))
                });
            deps.merge(cx.deps);
            result
        }
        Some(v) => Ok(v),
        None => Ok(Value::Unit),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CoreError {
    LangError(Box<OwnedResolvedError>),
    UnknownDeclaration {
        prefix: String,
        unknown: String,
        suggestions: Vec<String>,
    },
    Other(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LangError(owned_resolved_error) => owned_resolved_error.fmt(f),
            Self::UnknownDeclaration {
                prefix,
                unknown,
                suggestions,
            } => f.write_fmt(format_args!(
                "unknown declaration {unknown} after {prefix}, try one of {suggestions:?}"
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
                    ..Default::default()
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
