use std::{path::PathBuf, process::ExitCode};

use rain_lang::{ir::Rir, runner::Runner, tokens::peek::PeekTokenStream};

fn main() -> ExitCode {
    let Some(src_path) = std::env::args().nth(1) else {
        print_help();
        return ExitCode::FAILURE;
    };
    let src_path = PathBuf::from(&src_path);
    let src = match std::fs::read_to_string(&src_path) {
        Ok(src) => src,
        Err(err) => {
            print_help();
            eprintln!("src_path = {src_path:?}");
            eprintln!("{err:#}");
            return ExitCode::FAILURE;
        }
    };
    if inner(src_path, src).is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_help() {
    eprintln!("Usage: rain-run <src_path>");
}

fn inner(path: PathBuf, src: String) -> Result<(), ()> {
    let mut stream = PeekTokenStream::new(&src);
    let script = rain_lang::ast::parser::parse_module(&mut stream).map_err(|err| {
        eprintln!("{}", err.resolve(Some(&path), &src));
    })?;
    let mut rir = Rir::new();
    let mid = rir.insert_module(Some(path), src, script);
    let Some(main) = rir.resolve_global_declaration(mid, "main") else {
        panic!("main declaration not found")
    };
    let mut runner = Runner::new(rir);
    let value = runner.evaluate_and_call(main).map_err(|err| {
        eprintln!("{}", err.resolve_ir(&runner.rir));
    })?;
    println!("{value:?}");
    Ok(())
}
