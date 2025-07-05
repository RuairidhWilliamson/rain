use std::path::Path;

use rain_lang::runner::value::Value;

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
