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
    let v = rain_lang::run_stderr(script)?;
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
