#![expect(clippy::print_stdout)]

use std::process::ExitCode;

use rain_lang::{afs::file::File, ast::error::ParseError, local_span::ErrorLocalSpan};

fn main() -> ExitCode {
    let Some(src_path) = std::env::args().nth(1) else {
        print_help();
        return ExitCode::FAILURE;
    };
    let src_path = std::path::Path::new(&src_path);
    let path = match File::new_local(src_path) {
        Ok(path) => path,
        Err(err) => {
            log::error!("Path error");
            log::error!("{err:#}");
            return ExitCode::FAILURE;
        }
    };
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
        let resolved = err.resolve(&path, &src);
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
    let module = rain_lang::ast::parser::parse_module(src)?;
    let out = module.display(src);
    println!("{out}");
    Ok(())
}
