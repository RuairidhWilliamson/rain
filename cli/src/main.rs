#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod commands;
mod exe;
mod remote;
mod run;

use std::collections::HashMap;
use std::path::PathBuf;
use std::{ffi::OsStr, process::ExitCode};

use clap::{Parser, Subcommand};
use env_logger::Env;
use rain_core::config::Config;
use remote::client::ClientMode;

fn main() -> ExitCode {
    if fallible_main().is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn fallible_main() -> Result<(), ()> {
    let config = rain_core::config::Config::default();
    if std::env::var_os("RAIN_SERVER").as_deref() == Some(OsStr::new("1")) {
        env_logger::init_from_env(Env::new().filter_or("RAIN_LOG", "debug"));
        return remote::server::rain_server(config).map_err(|err| {
            eprintln!("rain server error: {err:?}");
        });
    }
    env_logger::init_from_env(Env::new().filter("RAIN_LOG"));
    ctrlc::set_handler(|| {
        println!("\nCTRL+C pressed");
        std::process::exit(1);
    })
    .expect("init signal handler");
    let cli = Cli::parse();
    let mode = ClientMode::BackgroundThread;
    cli.main(&config, mode)
}

#[derive(Debug, Clone, Parser)]
struct GlobalOptions {
    /// Disable performing actions that require an internet connection and try to use cache more often
    #[arg(long, global = true, env = "RAIN_OFFLINE")]
    offline: bool,
    /// Override the host to a custom triple
    #[arg(long, global = true, env = "RAIN_HOST")]
    host: Option<String>,
    /// Resolve returned file paths before printing them to stdout
    #[arg(long, global = true)]
    resolve: bool,
    /// Disable escape commands (not a security sandbox)
    #[arg(long, global = true, env = "RAIN_SEAL")]
    seal: bool,
    /// The reporting mode to use
    #[arg(long, global = true, default_value = "basic")]
    report: ReportMode,
    /// The path to the rain source file entrypoint, if not specified will auto resolve main.rain
    #[arg(long, global = true)]
    entrypoint: Option<PathBuf>,

    #[arg(long, global = true)]
    config: Vec<String>,

    #[arg(long, global = true)]
    deps: bool,
}

impl GlobalOptions {
    fn parse_config(&self) -> Result<HashMap<String, String>, ()> {
        self.config
            .iter()
            .map(|v| {
                let Some((k, v)) = v.split_once('=') else {
                    eprintln!("config name and value must be separated by '='");
                    return Err(());
                };
                Ok((k.to_owned(), v.to_owned()))
            })
            .collect()
    }

    fn resolve_entrypoint(&self) -> Result<PathBuf, ()> {
        if let Some(entrypoint) = &self.entrypoint {
            Ok(entrypoint.clone())
        } else {
            rain_core::find_main_rain()
                .ok_or(())
                .map_err(|()| eprintln!("no main.rain found"))
        }
    }
}

#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[command(flatten)]
    options: GlobalOptions,
    #[command(subcommand)]
    command: RainCtlCommand,
}

#[derive(Debug, Subcommand)]
enum RainCtlCommand {
    /// Create a basic main.rain file in the current directory
    Init,
    /// Get information about the running rain server process
    Info,
    /// Run checks
    /// Equivalent to `rain exec check`
    Check,
    /// Build!
    /// Equivalent to `rain exec build`
    Build,
    /// Execute a rain function
    Exec {
        target: Option<String>,
        args: Vec<String>,
    },
    /// Stop the rain server process
    Shutdown,
    /// View rain config
    Config,
    /// Inspect the rain cache
    Cache,
    /// Resolve rain path to its actual local path
    Resolve { path: Option<String> },
    /// Clean the rain cache
    Clean,
    /// Prune the rain cache
    Prune,
}

#[test]
fn validate_cli() {
    <Cli as clap::CommandFactory>::command().debug_assert();
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
enum ReportMode {
    #[default]
    Basic,
    Verbose,
    None,
}

impl Cli {
    fn main(self, config: &Config, client_mode: ClientMode) -> Result<(), ()> {
        match self.command {
            RainCtlCommand::Init => commands::init_template(),
            RainCtlCommand::Check => run::run(config, "check", vec![], &self.options, client_mode),
            RainCtlCommand::Build => run::run(config, "build", vec![], &self.options, client_mode),
            RainCtlCommand::Exec { target, args } => run::run(
                config,
                &target.unwrap_or_default(),
                args,
                &self.options,
                client_mode,
            ),
            RainCtlCommand::Info => commands::info(config, client_mode),
            RainCtlCommand::Shutdown => commands::shutdown(config, client_mode),
            RainCtlCommand::Config => {
                commands::inspect_config(config);
                Ok(())
            }
            RainCtlCommand::Cache => commands::inspect_cache(config, client_mode),
            RainCtlCommand::Resolve { path } => {
                commands::resolve(config, path);
                Ok(())
            }
            RainCtlCommand::Clean => commands::clean(config, client_mode),
            RainCtlCommand::Prune => commands::prune(config, client_mode),
        }
    }
}
