#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use env_logger::Env;
use rain_core::{
    config::Config,
    remote::{
        client::make_request_or_start,
        msg::{info::InfoRequest, shutdown::ShutdownRequest},
    },
};

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
    let config = rain_core::config::Config::default();
    if std::env::var_os("RAIN_SERVER").as_deref() == Some(OsStr::new("1")) {
        return rain_core::remote::server::rain_server(config).map_err(|err| {
            eprintln!("rain server error: {err:?}");
        });
    }
    let decision_mode = RainCtlDecisionMode::get()?;
    match decision_mode {
        RainCtlDecisionMode::Auto => {
            let process_arg = std::env::args_os().next().ok_or_else(|| {
                eprintln!("not enough process args");
            })?;
            let p = std::path::Path::new(&process_arg);
            let exe_name = p.file_stem().ok_or_else(|| {
                eprintln!("this process bad exe path");
            })?;
            if exe_name == "rain" {
                rain_command(config)
            } else {
                rain_ctl_command(config)
            }
        }
        RainCtlDecisionMode::Always => rain_ctl_command(config),
        RainCtlDecisionMode::Never => rain_command(config),
    }
}

fn rain_command(config: Config) -> Result<(), ()> {
    let root = rain_core::find_root_rain().ok_or(())?;
    let v = rain_core::run_stderr(root, "main", config)?;
    eprintln!("{v:?}");
    Ok(())
}

fn rain_ctl_command(config: Config) -> Result<(), ()> {
    let cli = Cli::parse();
    match cli.command {
        RainCtlCommand::Noctl(args) => {
            let Some((_, args)) = args.split_first() else {
                unreachable!("cannot remove first arg")
            };
            if !std::process::Command::new(rain_core::exe::current_exe().ok_or(())?)
                .env("RAIN_CTL", "never")
                .args(args)
                .status()
                .map_err(|err| {
                    eprintln!("Failed to start noctl self: {err:?}");
                })?
                .success()
            {
                return Err(());
            }
        }
        RainCtlCommand::Inspect {
            script,
            declaration,
        } => {
            let v = rain_core::run_stderr(script, &declaration, config)?;
            eprintln!("{v:?}");
        }
        RainCtlCommand::Info => {
            let info = make_request_or_start(&config, InfoRequest).map_err(|err| {
                eprintln!("{err:?}");
            })?;
            println!("{info:#?}");
        }
        RainCtlCommand::Shutdown => {
            make_request_or_start(&config, ShutdownRequest).map_err(|err| {
                eprintln!("{err:?}");
            })?;
            println!("Server shutdown");
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
    Info,
    Shutdown,
    /// View and manipulate rain config
    Config,
    /// Clean the rain cache
    Clean,
}
