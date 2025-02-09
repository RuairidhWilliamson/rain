use std::path::Path;

use rain_lang::runner::value::Value;
use test_log::test;

fn run(path: impl AsRef<Path>) -> Result<Value, ()> {
    rain_core::run_log(path, "main")
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
