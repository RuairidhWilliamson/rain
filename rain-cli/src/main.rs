use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

fn main() -> ExitCode {
    if fallible_main().is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn fallible_main() -> Result<(), ()> {
    let cli = Cli::parse();
    let RainCommand::Run { script } = cli.command;
    let path = script;
    let src = std::fs::read_to_string(&path).map_err(|err| {
        eprintln!("{err}");
    })?;
    let ast = rain_lang::ast::parser::parse_module(&src);
    let mut rir = rain_lang::ir::Rir::new();
    let mid = rir.insert_module(Some(path), src, ast).map_err(|err| {
        eprintln!("{}", err.resolve_ir(&rir));
    })?;
    let main = rir.resolve_global_declaration(mid, "main").ok_or_else(|| {
        eprintln!("main declaration not found, add `let main` or `fn main() {{}}`",);
    })?;
    let mut runner = rain_lang::runner::Runner::new(rir);
    let v = runner.evaluate_and_call(main).map_err(|err| {
        eprintln!("{}", err.resolve_ir(&runner.rir));
    })?;
    eprintln!("{v:?}");
    Ok(())
}

#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: RainCommand,
}

#[derive(Debug, Subcommand)]
pub enum RainCommand {
    Run { script: PathBuf },
}
