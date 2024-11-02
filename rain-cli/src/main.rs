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
        RainCommand::RunScript { script } => {
            let v = rain_lang::run_stderr(script, rain_lang::config::Config::default())?;
            eprintln!("{v:?}");
            Ok(())
        }
        RainCommand::Clean => {
            let config = rain_lang::config::Config::default();
            let clean_path = &config.base_cache_dir;
            eprintln!("removing {}", clean_path.display());
            let metadata = match std::fs::metadata(clean_path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    eprintln!("could not stat cache directory: {err}");
                    return Err(());
                }
            };
            if !metadata.is_dir() {
                eprintln!("failed {} is not a directory", clean_path.display());
                return Err(());
            }
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
    RunScript { script: PathBuf },
    Clean,
}
