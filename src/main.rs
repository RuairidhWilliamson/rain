use std::path::PathBuf;

use clap::Parser;
use rain::ast::Script;

#[derive(Parser)]
struct Cli {
    script: PathBuf,

    #[arg(long)]
    print_tokens: bool,

    #[arg(long)]
    print_ast: bool,
}

fn main() {
    let cli = Cli::parse();

    let source = std::fs::read_to_string(cli.script).unwrap();
    let mut token_stream = rain::tokens::TokenStream::new(&source);
    let tokens = token_stream.parse_collect().unwrap();
    if cli.print_tokens {
        println!("{tokens:#?}");
    }

    let script = Script::parse(&tokens).unwrap();
    if cli.print_ast {
        println!("{script:#?}");
    }

    rain::exec::execute(&script).unwrap();
}
