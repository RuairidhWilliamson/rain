use std::{
    path::{Path, PathBuf},
    process::exit,
};

use clap::Parser;
use rain::{ast::script::Script, error::RainError};

#[derive(Parser)]
struct Cli {
    script: Option<PathBuf>,

    #[arg(long)]
    no_exec: bool,

    #[arg(long)]
    print_ast: bool,

    #[arg(long)]
    sealed: bool,
}

fn main() {
    let cli = Cli::parse();

    let path = cli
        .script
        .as_deref()
        .unwrap_or_else(|| Path::new("main.rain"));
    let source = std::fs::read_to_string(path).unwrap();
    if let Err(err) = main_inner(&source, &cli) {
        let err = err.resolve(path, &source);
        eprintln!("{err:#}");
        exit(1)
    }
}

fn main_inner(source: impl Into<String>, cli: &Cli) -> Result<(), RainError> {
    // TODO: We should properly track the lifetime of the source code
    let source = Into::<String>::into(source).leak();
    let mut token_stream = rain::tokens::peek_stream::PeekTokenStream::new(source);

    let script = Script::parse_stream(&mut token_stream)?;
    if cli.print_ast {
        println!("{script:#?}");
    }

    if !cli.no_exec {
        let options = rain::exec::ExecuteOptions { sealed: cli.sealed };
        rain::exec::execute(&script, options)?;
    }
    Ok(())
}
