use std::process::ExitCode;

use rain_lang::{
    ast::error::ParseError, error::ErrorSpan, ir::Rir, runner::Runner,
    tokens::peek::PeekTokenStream,
};

fn main() -> ExitCode {
    let Some(src_path) = std::env::args().nth(1) else {
        print_help();
        return ExitCode::FAILURE;
    };
    let src_path = std::path::Path::new(&src_path);
    let src = match std::fs::read_to_string(src_path) {
        Ok(src) => src,
        Err(err) => {
            print_help();
            eprintln!("src_path = {src_path:?}");
            eprintln!("{err:#}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = inner(src_path, &src) {
        let resolved = err.resolve(Some(src_path), &src);
        eprintln!("{resolved}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_help() {
    eprintln!("Usage: rain-run <src_path>");
}

fn inner(path: &std::path::Path, src: &str) -> Result<(), ErrorSpan<ParseError>> {
    let mut stream = PeekTokenStream::new(src);
    let script = rain_lang::ast::Script::parse(&mut stream)?;
    let mut rir = Rir::new();
    let modid = rir.insert_module(Some(path), src, &script);
    let Some(main) = rir.resolve_global_declaration(modid, "main") else {
        panic!("no main")
    };
    let mut runner = Runner::new(&rir);
    let value = runner.evaluate(main);
    println!("{value:?}");
    Ok(())
}
