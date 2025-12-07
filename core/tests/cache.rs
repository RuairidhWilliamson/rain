#![cfg(test)]

use std::{
    fs::{self},
    io::{Seek as _, Write as _},
    path::Path,
    sync::Arc,
};

use poison_panic::MutexExt as _;
use rain_core::cache::persistent::PersistCache;
use rain_lang::{
    afs::entry::FSEntryTrait as _,
    driver::FSTrait as _,
    runner::value::{RainInteger, Value},
};
use test_log::test;

struct CacheTester {
    config: rain_core::config::Config,
    driver: rain_core::driver::DriverImpl<'static>,
    persist_cache: Option<PersistCache>,
    cache_stats: rain_core::cache::CacheStats,
}

impl CacheTester {
    fn new() -> Self {
        let config = rain_core::config::Config::new();
        let driver = rain_core::driver::DriverImpl::new(config.clone());
        let persist_cache = None;
        let stats = rain_core::cache::CacheStats::default();
        Self {
            config,
            driver,
            persist_cache,
            cache_stats: stats,
        }
    }

    fn run(&mut self, path: impl AsRef<Path>, declaration: &str) -> Value {
        let file = rain_lang::afs::file::File::new_local(path.as_ref()).unwrap();
        let path = self.driver.resolve_fs_entry(file.inner());
        let src = std::fs::read_to_string(&path).unwrap();
        let module = rain_lang::ast::parser::parse_module(&src);
        let mut ir = rain_lang::ir::Rir::new();
        let cache_core = self.persist_cache.take().unwrap_or_default().depersist(
            &self.config,
            &self.cache_stats,
            &mut ir,
        );
        let cache = rain_core::cache::Cache::new(cache_core);
        let mid = ir.insert_module(Some(file), src, module).unwrap();
        let main = ir.resolve_global_declaration(mid, declaration).unwrap();
        let mut runner = rain_lang::runner::Runner::new(&mut ir, &cache, &self.driver);
        let value = runner.evaluate_and_call(main, &[]).unwrap();
        self.persist_cache = Some(PersistCache::persist(
            &cache.core.plock(),
            &cache.stats,
            &ir,
        ));
        value
    }
}

#[test]
fn modify_local_root() {
    let mut cache_tester = CacheTester::new();

    let mut f = tempfile::NamedTempFile::new().unwrap();

    writeln!(f, "let x = 5\nlet main = x").unwrap();
    f.flush().unwrap();
    f.seek(std::io::SeekFrom::Start(0)).unwrap();
    let value = cache_tester.run(&f, "main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(5))));

    writeln!(f, "let x = 6\nlet main = x").unwrap();
    f.flush().unwrap();
    f.seek(std::io::SeekFrom::Start(0)).unwrap();
    let value = cache_tester.run(&f, "main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(6))));
}

#[test]
fn modify_local_import() {
    let mut cache_tester = CacheTester::new();

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("main.rain");
    fs::write(
        &root,
        "let child = internal._import(internal._get_file(\"child.rain\"))
        let main = child.x",
    )
    .unwrap();
    let child = dir.path().join("child.rain");
    fs::write(&child, "let x = 4").unwrap();

    let value = cache_tester.run(&root, "main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(4))));

    fs::write(&child, "let x = 5").unwrap();
    let value = cache_tester.run(&root, "main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(5))));
}
