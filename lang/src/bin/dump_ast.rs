#![expect(clippy::print_stdout)]

use std::process::ExitCode;

use rain_lang::{
    ast::error::ParseError,
    local_span::{ErrorLocalSpan, LocalSpan},
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
            println!("src_path = {src_path:?}");
            println!("{err:#}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = inner(&src) {
        let resolved = err.resolve(None, &src);
        println!("{resolved}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_help() {
    println!("Usage: dump_ast <src_path>");
}

fn inner(src: &str) -> Result<(), ErrorLocalSpan<ParseError>> {
    let module = rain_lang::ast::ts_parser::parse_module(src).map_err(|err| match err {
        rain_lang::ast::ts_parser::Error::TreeSitter
        | rain_lang::ast::ts_parser::Error::DepthLimit => {
            LocalSpan::byte(0).with_error(ParseError::TreeSitter)
        }
        rain_lang::ast::ts_parser::Error::ParseErrors(items) => {
            let (span, err) = items.first().unwrap();
            eprintln!("{err}");
            span.with_error(ParseError::TreeSitter)
        }
    })?;
    let out = module.display(src);
    println!("{out}");
    Ok(())
}
