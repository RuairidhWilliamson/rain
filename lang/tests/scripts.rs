use std::path::Path;

use rain_lang::{
    ast::Script,
    ir::Rir,
    runner::{value::RainValue, Runner},
    tokens::peek::PeekTokenStream,
};

fn run_inner(path: Option<&Path>, src: &str) -> anyhow::Result<RainValue> {
    let mut stream = PeekTokenStream::new(src);
    let ast = Script::parse(&mut stream).map_err(|err| {
        eprintln!("{}", err.resolve(path, src));
        err.err
    })?;
    let mut ir = Rir::new();
    let module_id = ir.insert_module(path, src, &ast);
    let main = ir
        .resolve_global_declaration(module_id, "main")
        .ok_or_else(|| anyhow::anyhow!("main declaration not found"))?;
    let mut runner = Runner::new(&ir);
    let value = runner.evaluate_and_call(main).map_err(|err| {
        eprintln!("{}", err.resolve(path, src));
        err.err
    })?;
    Ok(value)
}

fn run(path: impl AsRef<Path>) -> anyhow::Result<RainValue> {
    let path: &Path = path.as_ref();
    let src = std::fs::read_to_string(path)?;
    run_inner(Some(path), &src)
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
