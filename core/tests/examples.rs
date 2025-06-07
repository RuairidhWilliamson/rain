use std::path::Path;

use rain_core::CoreError;
use rain_lang::runner::value::Value;
use test_log::test;

fn run(path: impl AsRef<Path>) -> Result<Value, ()> {
    rain_core::run_stderr(path, "main")
}

#[test]
fn hello_world() {
    run("examples/helloworld/hello.rain").unwrap();
}

#[test]
fn imports() {
    run("examples/imports/test.rain").unwrap();
}

#[test]
fn areas() {
    run("examples/areas/area.rain").unwrap();
}

#[test]
fn error_throwing() {
    let driver = rain_core::driver::DriverImpl::new(rain_core::config::Config::default());
    let cache = rain_core::cache::Cache::default();
    let res = rain_core::run("examples/errors/throwing.rain", "main", &cache, &driver);
    match res {
        Err(CoreError::LangError(err)) => {
            assert_eq!(err.err, "\"test\"");
        }
        _ => panic!("wrong error {res:?}"),
    }
}
