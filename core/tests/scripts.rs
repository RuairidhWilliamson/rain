use std::path::Path;

use rain_core::{
    cache::{Cache, CacheStats, persistent::PersistCache},
    driver::DriverImpl,
};
use rain_lang::driver::FSTrait;
use rain_lang::{afs::entry::FSEntryTrait as _, runner::value::Value};
use test_log::test;

fn run(path: impl AsRef<Path>) -> Result<Value, ()> {
    rain_core::run_stderr(path, "main")
}

#[test]
fn utf8() {
    insta::assert_debug_snapshot!(run("tests/scripts/utf-8.rain").unwrap());
}

#[test]
fn fib() {
    insta::assert_debug_snapshot!(run("tests/scripts/fib.rain").unwrap());
}

#[test]
fn local_var() {
    insta::assert_debug_snapshot!(run("tests/scripts/local_var.rain").unwrap());
}

#[test]
fn fn_call() {
    insta::assert_debug_snapshot!(run("tests/scripts/fn_call.rain").unwrap());
}

#[test]
fn internal_print() {
    insta::assert_debug_snapshot!(run("tests/scripts/internal_print.rain").unwrap());
}

#[test]
fn internal_import() {
    insta::assert_debug_snapshot!(run("tests/scripts/internal_import.rain").unwrap());
}

#[test]
fn underscore() {
    insta::assert_debug_snapshot!(run("tests/scripts/underscore.rain").unwrap());
}

#[test]
fn equality() {
    insta::assert_debug_snapshot!(run("tests/scripts/equality.rain").unwrap());
}

#[test]
fn create_area() {
    insta::assert_debug_snapshot!(run("tests/scripts/create_area.rain").unwrap());
}

#[test]
fn cache_deps() {
    insta::assert_debug_snapshot!(run("tests/scripts/cache_deps.rain").unwrap());
}

#[test]
fn import_cache() {
    let cache = Cache::default();
    let config = rain_core::config::Config::default();
    let driver = DriverImpl::new(config.clone());
    let file = rain_lang::afs::file::File::new_local("tests/scripts/cache.rain".as_ref()).unwrap();
    let path = driver.resolve_fs_entry(file.inner());
    let src = std::fs::read_to_string(&path).unwrap();
    let module = rain_lang::ast::parser::parse_module(&src);
    let mut ir = rain_lang::ir::Rir::new();
    let mid = ir.insert_module(Some(file), src, module).unwrap();
    let main = ir.resolve_global_declaration(mid, "main").unwrap();
    let mut runner = rain_lang::runner::Runner::new(&mut ir, &cache, &driver);

    let value = runner.evaluate_and_call(main, &[]).unwrap();
    assert_eq!(format!("{value:?}"), "Module(ModuleId(1))");

    // intra process cache
    let value = runner.evaluate_and_call(main, &[]).unwrap();
    assert_eq!(format!("{value:?}"), "Module(ModuleId(1))");

    drop(runner);
    // inter process cache
    let persistent_cache = PersistCache::persist(&*cache.core.lock().unwrap(), &cache.stats, &ir);
    drop(cache);

    let stats = CacheStats::default();
    let cache_core = persistent_cache.depersist(&config, &stats, &mut ir);
    let cache = Cache::new(cache_core);
    let mut runner = rain_lang::runner::Runner::new(&mut ir, &cache, &driver);

    let value = runner.evaluate_and_call(main, &[]).unwrap();
    // TODO: Check this value was retrieved from the cache
    assert_eq!(format!("{value:?}"), "Module(ModuleId(2))");
}
