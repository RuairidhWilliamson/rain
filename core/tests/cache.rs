#![cfg(test)]

use std::{
    fs,
    io::{Seek as _, SeekFrom, Write as _},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use alias::Alias as _;
use poison_panic::MutexExt as _;
use rain_core::cache::{Cache, persistent::PersistCache};
use rain_lang::{
    afs::{File, local::file::LocalFile},
    ast::Module,
    cancellation::Cancellation,
    driver::FSTrait as _,
    runner::{
        dep_list::DepList,
        value::{RainInteger, Value},
    },
};
use tracing_test::traced_test;

struct CacheTester {
    config: rain_core::config::Config,
    driver: rain_core::driver::DriverImpl<'static>,
    persist_cache: Option<PersistCache>,
    cache_stats: Arc<rain_core::cache::CacheStats>,
}

impl CacheTester {
    fn new() -> Self {
        let config = rain_core::config::Config::new();
        let driver = rain_core::driver::DriverImpl::new(config.clone());
        let persist_cache = None;
        let stats = Arc::new(rain_core::cache::CacheStats::default());
        Self {
            config,
            driver,
            persist_cache,
            cache_stats: stats,
        }
    }

    fn run(&mut self, path: impl AsRef<Path>) -> CacheTesterRun<'_> {
        let file = LocalFile::new_local(&self.driver, path.as_ref()).unwrap();
        let path = self.driver.resolve_fs_entry(file.fsinner().into());
        let src = std::fs::read_to_string(&path).unwrap();
        let module = Module::parse(&src);
        let mut ir = rain_lang::ir::Rir::new();
        let cache_core = self.persist_cache.take().unwrap_or_default().depersist(
            &self.config,
            &self.cache_stats,
            &mut ir,
        );
        let cache = Cache {
            execution_time_thresold: Duration::ZERO,
            core: Arc::new(Mutex::new(cache_core)),
            stats: self.cache_stats.alias(),
            verification: false,
        };
        let mid = ir
            .insert_module(Some(File::Local(file)), src, module)
            .unwrap();
        CacheTesterRun {
            tester: self,
            ir,
            cache,
            mid,
        }
    }
}

struct CacheTesterRun<'a> {
    tester: &'a mut CacheTester,
    ir: rain_lang::ir::Rir,
    cache: Cache,
    mid: rain_lang::ir::ModuleId,
}

impl CacheTesterRun<'_> {
    fn exec(&mut self, target: &str) -> Value {
        let mut runner = rain_lang::runner::Runner::new(
            &mut self.ir,
            &self.cache,
            &self.tester.driver,
            Cancellation::new(),
        );
        let mut deps = DepList::new();
        rain_core::evaluate_and_call_chain(&mut runner, self.mid, &mut deps, target, &[]).unwrap()
    }
}

impl Drop for CacheTesterRun<'_> {
    fn drop(&mut self) {
        self.tester.persist_cache = Some(PersistCache::persist(
            &self.cache.core.plock(),
            &self.cache.stats,
            &self.ir,
        ));
    }
}

#[traced_test]
#[test]
fn unchanged_local_file_declaration() {
    let mut tester = CacheTester::new();

    let mut f = tempfile::NamedTempFile::new().unwrap();
    let counter_name = Arc::new(String::from("foo"));

    write!(
        f,
        "
        let main = internal._inc_counter(\"foo\")
        "
    )
    .unwrap();
    f.flush().unwrap();
    f.seek(SeekFrom::Start(0)).unwrap();

    {
        let mut run = tester.run(&f);

        let value = run.exec("main");
        assert_eq!(value, Value::Unit);
        assert_eq!(1, run.tester.driver.get_counter(&counter_name));

        // Same run still cached
        let value = run.exec("main");
        assert_eq!(value, Value::Unit);
        assert_eq!(1, run.tester.driver.get_counter(&counter_name));
    }

    // New run, should still be cached
    let value = tester.run(&f).exec("main");
    assert_eq!(value, Value::Unit);
    assert_eq!(1, tester.driver.get_counter(&counter_name));
}

#[traced_test]
#[test]
fn modify_local_root() {
    let mut tester = CacheTester::new();

    let mut f = tempfile::NamedTempFile::new().unwrap();

    write!(f, "let main = 5").unwrap();
    f.flush().unwrap();
    f.seek(SeekFrom::Start(0)).unwrap();
    let value = tester.run(&f).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(5))));

    write!(f, "let main = 6").unwrap();
    f.flush().unwrap();
    f.seek(SeekFrom::Start(0)).unwrap();
    let value = tester.run(&f).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(6))));
}

#[traced_test]
#[test]
fn modify_local_import() {
    let mut tester = CacheTester::new();

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("main.rain");
    fs::write(
        &root,
        "let child = internal._import(internal._get_file(\"child.rain\"))
        let main = child.x",
    )
    .unwrap();
    let child = dir.path().join("child.rain");
    fs::write(&child, "pub let x = 4").unwrap();

    let value = tester.run(&root).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(4))));

    fs::write(&child, "pub let x = 5").unwrap();
    let value = tester.run(&root).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(5))));
}

#[traced_test]
#[test]
fn unchanged_local_import() {
    let mut tester = CacheTester::new();
    let counter_name = Arc::new(String::from("foo"));

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("main.rain");
    fs::write(
        &root,
        "
        let child = internal._import(internal._get_file(\"child.rain\"))
        let main = child.x()
        ",
    )
    .unwrap();
    let child = dir.path().join("child.rain");
    fs::write(
        &child,
        "
        pub let x = fn() {
            internal._inc_counter(\"foo\")
            5
        }
    ",
    )
    .unwrap();

    let value = tester.run(&root).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(5))));
    assert_eq!(1, tester.driver.get_counter(&counter_name));

    // Should cache since child.rain did not change
    let value = tester.run(&root).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(5))));
    assert_eq!(1, tester.driver.get_counter(&counter_name));

    // Change child.rain
    fs::write(
        &child,
        "
        pub let x = fn() {
            internal._inc_counter(\"foo\")
            6
        }
    ",
    )
    .unwrap();

    // Should not cache since child.rain has changed
    let value = tester.run(&root).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(6))));
    assert_eq!(2, tester.driver.get_counter(&counter_name));
}

#[traced_test]
#[test]
fn non_capturing_closure_caching() {
    let mut tester = CacheTester::new();
    let counter_name = Arc::new(String::from("foo"));

    let f = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        &f,
        r#"
        let main = fn() {
            internal._set_cache_never()
            foo()()
        }

        let foo = fn() {
            fn() {
                internal._inc_counter("foo")
                42
            }
        }
        "#,
    )
    .unwrap();
    let value = tester.run(&f).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(42))));
    assert_eq!(1, tester.driver.get_counter(&counter_name));

    // Should be cached
    let value = tester.run(&f).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(42))));
    assert_eq!(1, tester.driver.get_counter(&counter_name));
}

#[traced_test]
#[test]
fn capturing_closure_caching() {
    let mut tester = CacheTester::new();
    let counter_name = Arc::new(String::from("foo"));

    let f = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        &f,
        r#"
        let main = fn() {
            internal._set_cache_never()
            foo(1)()
        }

        let foo = fn(x) {
            fn() {
                internal._inc_counter("foo")
                42 + x
            }
        }
        "#,
    )
    .unwrap();
    let value = tester.run(&f).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(43))));
    assert_eq!(1, tester.driver.get_counter(&counter_name));

    // Should be cached
    let value = tester.run(&f).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(43))));
    assert_eq!(1, tester.driver.get_counter(&counter_name));
}

#[traced_test]
#[test]
fn capturing_closure_across_modules_caching() {
    let mut tester = CacheTester::new();

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("main.rain");
    fs::write(
        &root,
        "
        let child = internal._import(internal._get_file(\"child.rain\"))
        let foo = fn() {
            x = child.x
            fn() {
                x + 1
            }
        }
        let main = foo()()
        ",
    )
    .unwrap();
    let child = dir.path().join("child.rain");
    fs::write(&child, "pub let x = 4").unwrap();

    let value = tester.run(&root).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(5))));

    fs::write(&child, "pub let x = 5").unwrap();
    let value = tester.run(&root).exec("main");
    assert_eq!(value, Value::Integer(Arc::new(RainInteger::from(6))));
}
