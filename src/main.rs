use std::path::PathBuf;

use clap::Parser;
use rain::ast::Script;

#[derive(Parser)]
struct Cli {
    script: Option<PathBuf>,

    #[arg(long)]
    print_tokens: bool,

    #[arg(long)]
    print_ast: bool,

    #[arg(long)]
    sealed: bool,
}

fn main() {
    let cli = Cli::parse();

    let path = cli.script.unwrap_or_else(|| PathBuf::from("main.rain"));
    let source = std::fs::read_to_string(&path).unwrap();
    let mut token_stream = rain::tokens::TokenStream::new(&source);
    let tokens = token_stream.parse_collect().unwrap();
    if cli.print_tokens {
        println!("{tokens:#?}");
    }

    let script = Script::parse(&tokens).unwrap();
    if cli.print_ast {
        println!("{script:#?}");
    }

    let options = rain::exec::ExecuteOptions { sealed: cli.sealed };

    if let Err(err) = rain::exec::execute(&script, options) {
        let err = err.resolve(&path, &source);
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
