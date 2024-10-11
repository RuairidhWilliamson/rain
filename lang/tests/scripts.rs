use std::path::PathBuf;

use rain_lang::{
    ir::Rir,
    runner::{value::Value, Runner},
};

fn run_inner(path: Option<PathBuf>, src: String) -> anyhow::Result<Value> {
    let module = rain_lang::ast::parser::parse_module(&src);
    let mut ir = Rir::new();
    let mid = ir.insert_module(path, src, module).map_err(|err| {
        eprintln!("{}", err.resolve_ir(&ir));
        err.err
    })?;
    let main = ir
        .resolve_global_declaration(mid, "main")
        .ok_or_else(|| anyhow::anyhow!("main declaration not found"))?;
    let mut runner = Runner::new(ir);
    let value = runner.evaluate_and_call(main).map_err(|err| {
        eprintln!("{}", err.resolve_ir(&runner.rir));
        err.err
    })?;
    Ok(value)
}

fn run(path: impl Into<PathBuf>) -> anyhow::Result<Value> {
    let path: PathBuf = path.into();
    let src = std::fs::read_to_string(&path)?;
    run_inner(Some(path), src)
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
