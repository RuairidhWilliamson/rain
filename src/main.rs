use std::{
    path::{Path, PathBuf},
    process::exit,
};

use clap::Parser;
use rain::{ast::Script, error::RainError};

#[derive(Parser)]
struct Cli {
    script: Option<PathBuf>,

    #[arg(long)]
    no_exec: bool,

    #[arg(long)]
    print_tokens: bool,

    #[arg(long)]
    print_ast: bool,

    #[arg(long)]
    sealed: bool,
}

fn main() {
    let cli = Cli::parse();

    let path = cli
        .script
        .as_ref()
        .map(|p| p.as_path())
        .unwrap_or_else(|| Path::new("main.rain"));
    let source = std::fs::read_to_string(&path).unwrap();
    if let Err(err) = main_inner(&source, &cli) {
        let err = err.resolve(&path, &source);
        eprintln!("{err:#}");
        exit(1)
    }
}

fn main_inner(source: &str, cli: &Cli) -> Result<(), RainError> {
    let mut token_stream = rain::tokens::TokenStream::new(&source);
    let tokens = token_stream.parse_collect()?;
    if cli.print_tokens {
        println!("{tokens:#?}");
    }

    let script = Script::parse(&tokens)?;
    if cli.print_ast {
        println!("{script:#?}");
    }

    if !cli.no_exec {
        let options = rain::exec::ExecuteOptions { sealed: cli.sealed };
        rain::exec::execute(&script, options)?;
    }
    Ok(())
}
