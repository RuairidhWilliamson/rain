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
    match cli.command {
        RainCommand::Run { script } => {
            let v = rain_lang::run_stderr(script, rain_lang::config::Config::default())?;
            eprintln!("{v:?}");
            Ok(())
        }
        RainCommand::Clean => {
            let config = rain_lang::config::Config::default();
            let clean_path = &config.base_cache_dir;
            eprintln!("Removing {}", clean_path.display());
            if let Err(err) = std::fs::remove_dir_all(clean_path) {
                eprintln!("clean failed: {err}");
                return Err(());
            }
            Ok(())
        }
    }
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
    Clean,
}
