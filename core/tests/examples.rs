use std::path::Path;

use rain_core::runner::value::Value;

fn run(path: impl AsRef<Path>) -> Result<Value, ()> {
    rain_core::run_stderr(path, "main", rain_core::config::Config::default())
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
