use std::process::ExitCode;

use rain_lang::{
    ast::{display::display_ast, error::ParseError},
    error::ErrorSpan,
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
    if let Err(err) = inner(&src) {
        let resolved = err.resolve(src_path, &src);
        eprintln!("{resolved}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_help() {
    eprintln!("Usage: dump_ast <src_path>");
}

fn inner(src: &str) -> Result<(), ErrorSpan<ParseError>> {
    let mut stream = PeekTokenStream::new(src);
    let script = rain_lang::ast::Script::parse(&mut stream)?;
    let out = display_ast(&script, src);
    println!("{out}");
    Ok(())
}
