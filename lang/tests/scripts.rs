use std::path::Path;

use rain_lang::{ast::Script, ir::Rir, runner::Runner, tokens::peek::PeekTokenStream};

#[test]
fn run_all_test_scripts() {
    let test_scripts_dir = std::fs::read_dir("tests/scripts").unwrap();
    let mut error_count = 0;
    test_scripts_dir.for_each(|test_script| {
        let test_script = test_script.unwrap();
        let path = test_script.path();
        if let Err(err) = run_inner(&path) {
            eprintln!("{err:#}");
            error_count += 1;
        }
    });
    if error_count > 0 {
        panic!("{error_count} errors");
    }
}

fn run_inner(path: &Path) -> anyhow::Result<()> {
    let src = std::fs::read_to_string(path).unwrap();
    let mut stream = PeekTokenStream::new(&src);
    let ast = Script::parse(&mut stream).map_err(|err| {
        eprintln!("{}", err.resolve(Some(path), &src));
        err.err
    })?;
    let mut ir = Rir::new();
    let module_id = ir.insert_module(None, &src, &ast);
    let main = ir
        .resolve_global_declaration(module_id, "main")
        .ok_or(anyhow::anyhow!("main function not found"))?;
    let mut runner = Runner::new(&ir);
    let value = runner.evaluate(main);
    eprintln!("{value:?}");
    Ok(())
}
