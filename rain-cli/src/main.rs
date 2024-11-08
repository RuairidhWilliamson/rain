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
    let process_arg = std::env::args_os().next().ok_or_else(|| {
        eprintln!("not enough process args");
    })?;
    let p = std::path::Path::new(&process_arg);
    let exe_name = p.file_stem().ok_or_else(|| {
        eprintln!("this process bad exe path");
    })?;
    if exe_name == "rain" {
        rain_command()
    } else {
        rainx_command()
    }
}

fn rain_command() -> Result<(), ()> {
    let config = rain_lang::config::Config::default();
    let v = rain_lang::run_stderr("main.rain", "main", config)?;
    eprintln!("{v:?}");
    Ok(())
}

fn rainx_command() -> Result<(), ()> {
    let cli = Cli::parse();
    let config = rain_lang::config::Config::default();
    match cli.command {
        RainCommand::Inspect {
            script,
            declaration,
        } => {
            let v = rain_lang::run_stderr(script, &declaration, config)?;
            eprintln!("{v:?}");
        }
        RainCommand::Config => {
            eprintln!("{config:#?}");
        }
        RainCommand::Clean => {
            let clean_path = &config.base_cache_dir;
            eprintln!("removing {}", clean_path.display());
            let metadata = std::fs::metadata(clean_path).map_err(|err| {
                eprintln!("could not stat cache directory: {err}");
            })?;
            if !metadata.is_dir() {
                eprintln!("failed {} is not a directory", clean_path.display());
                return Err(());
            }
            std::fs::remove_dir_all(clean_path).map_err(|err| {
                eprintln!("clean failed: {err}");
            })?;
        }
    }
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
    Inspect {
        script: PathBuf,
        declaration: String,
    },
    /// View and manipulate rain config
    Config,
    /// Clean the rain cache
    Clean,
}
