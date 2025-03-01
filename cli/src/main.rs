#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::{ffi::OsStr, process::ExitCode};

use clap::{Parser, Subcommand};
use env_logger::Env;
use rain_core::{
    config::Config,
    remote::{
        client::make_request_or_start,
        msg::{
            clean::CleanRequest,
            info::InfoRequest,
            run::{RunProgress, RunRequest},
            shutdown::ShutdownRequest,
        },
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

fn fallible_main() -> Result<(), ()> {
    let config = rain_core::config::Config::default();
    if std::env::var_os("RAIN_SERVER").as_deref() == Some(OsStr::new("1")) {
        return rain_core::remote::server::rain_server(config).map_err(|err| {
            eprintln!("rain server error: {err:?}");
        });
    }
    rain_ctl_command(&config)
}

fn rain_ctl_command(config: &Config) -> Result<(), ()> {
    let cli = Cli::parse();
    match cli.command {
        RainCtlCommand::Run { target, monitor } => {
            let root = rain_core::find_root_rain().ok_or(())?;
            let run_response =
                make_request_or_start(config, RunRequest { root, target }, |im| match im {
                    RunProgress::Print(s) => println!("{s}"),
                    RunProgress::EnterCall(s) => {
                        if monitor {
                            println!("+ {s}");
                        }
                    }
                    RunProgress::ExitCall(s) => {
                        if monitor {
                            println!("- {s}");
                        }
                    }
                })
                .map_err(|err| {
                    eprintln!("{err}");
                })?;
            let result = run_response.output;
            match result {
                Ok(s) => {
                    println!("{s}");
                }
                Err(s) => {
                    match s {
                        rain_core::CoreError::LangError(owned_resolved_error) => {
                            let mut stdout =
                                termcolor::StandardStream::stdout(termcolor::ColorChoice::Auto);
                            owned_resolved_error
                                .write_color(&mut stdout)
                                .expect("write stdout");
                        }
                        rain_core::CoreError::Other(s) => {
                            println!("{s}");
                        }
                    }
                    return Err(());
                }
            }
        }
        RainCtlCommand::Info => {
            let info = make_request_or_start(config, InfoRequest, |()| {}).map_err(|err| {
                eprintln!("{err}");
            })?;
            println!("{info:#?}");
        }
        RainCtlCommand::Shutdown => {
            make_request_or_start(config, ShutdownRequest, |()| {}).map_err(|err| {
                eprintln!("{err}");
            })?;
            println!("Server shutdown");
        }
        RainCtlCommand::Config => {
            eprintln!("{config:#?}");
        }
        RainCtlCommand::Clean => {
            println!("Will delete:");
            for p in config.clean_directories() {
                println!("  {}", p.display());
            }
            if inquire::Confirm::new("Delete all these directories?")
                .prompt_skippable()
                .map_err(|err| {
                    eprintln!("{err}");
                })?
                == Some(true)
            {
                make_request_or_start(config, CleanRequest, |()| {}).map_err(|err| {
                    eprintln!("{err}");
                })?;
                println!("Cleaned");
            } else {
                println!("Did nothing");
            }
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
    Info,
    Run {
        target: String,

        #[arg(long)]
        monitor: bool,
    },
    Shutdown,
    /// View rain config
    Config,
    /// Clean the rain cache
    Clean,
}
