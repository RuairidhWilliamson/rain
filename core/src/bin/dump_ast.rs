use std::process::ExitCode;

use rain_core::{area::File, ast::error::ParseError, local_span::ErrorLocalSpan};

fn main() -> ExitCode {
    env_logger::init();
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
            log::error!("src_path = {src_path:?}");
            log::error!("{err:#}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = inner(&src) {
        let resolved = err.resolve(&path, &src);
        log::error!("{resolved}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_help() {
    log::info!("Usage: dump_ast <src_path>");
}

fn inner(src: &str) -> Result<(), ErrorLocalSpan<ParseError>> {
    let module = rain_core::ast::parser::parse_module(src)?;
    let out = module.display(src);
    log::info!("{out}");
    Ok(())
}
