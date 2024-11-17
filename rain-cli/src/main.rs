#![allow(clippy::print_stderr)]

use std::{ffi::OsString, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use env_logger::Env;

fn main() -> ExitCode {
    env_logger::init_from_env(Env::new().filter("RAIN_LOG"));
    if fallible_main().is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

enum RainCtlDecisionMode {
    Auto,
    Always,
    Never,
}

impl RainCtlDecisionMode {
    fn get() -> Result<Self, ()> {
        let env_var = match std::env::var("RAIN_CTL") {
            Ok(s) => Some(s),
            Err(std::env::VarError::NotPresent) => None,
            _ => {
                eprintln!("bad env var value for RAIN_CTL");
                return Err(());
            }
        };
        match env_var.as_deref() {
            Some("auto") | None => Ok(Self::Auto),
            Some("always") => Ok(Self::Always),
            Some("never") => Ok(Self::Never),
            Some(s) => {
                eprintln!("bad env var value for RAIN_CTL: {s:?}");
                Err(())
            }
        }
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
    let decision_mode = RainCtlDecisionMode::get()?;
    match decision_mode {
        RainCtlDecisionMode::Auto => {
            if exe_name == "rain" {
                rain_command()
            } else {
                rain_ctl_command()
            }
        }
        RainCtlDecisionMode::Always => rain_ctl_command(),
        RainCtlDecisionMode::Never => rain_command(),
    }
}

fn find_root_rain() -> Result<PathBuf, ()> {
    let mut directory = std::env::current_dir().unwrap();
    loop {
        let p = directory.join("root.rain");
        if p.try_exists().unwrap() {
            return Ok(p);
        }
        if !directory.pop() {
            return Err(());
        }
    }
}

fn rain_command() -> Result<(), ()> {
    let config = rain_lang::config::Config::default();
    let root = find_root_rain()?;
    let v = rain_lang::run_stderr(root, "main", config)?;
    eprintln!("{v:?}");
    Ok(())
}

fn rain_ctl_command() -> Result<(), ()> {
    let cli = Cli::parse();
    let config = rain_lang::config::Config::default();
    match cli.command {
        RainCtlCommand::Noctl(args) => {
            let Some((_, args)) = args.split_first() else {
                unreachable!("cannot remove first arg")
            };
            if !std::process::Command::new(std::env::current_exe().unwrap())
                .env("RAIN_CTL", "never")
                .args(args)
                .status()
                .unwrap()
                .success()
            {
                return Err(());
            }
        }
        RainCtlCommand::Inspect {
            script,
            declaration,
        } => {
            let v = rain_lang::run_stderr(script, &declaration, config)?;
            eprintln!("{v:?}");
        }
        RainCtlCommand::Config => {
            eprintln!("{config:#?}");
        }
        RainCtlCommand::Clean => {
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
    command: RainCtlCommand,
}

#[derive(Debug, Subcommand)]
pub enum RainCtlCommand {
    #[command(external_subcommand)]
    Noctl(Vec<OsString>),
    Inspect {
        script: PathBuf,
        declaration: String,
    },
    /// View and manipulate rain config
    Config,
    /// Clean the rain cache
    Clean,
}
