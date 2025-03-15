#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::{
    ffi::OsStr,
    io::{Write as _, stdout},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use env_logger::Env;
use rain_core::{
    config::Config,
    remote::{
        client::make_request_or_start,
        msg::{
            clean::CleanRequest,
            info::InfoRequest,
            inspect::InspectRequest,
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
        RainCtlCommand::Run { target } => run(config, target),
        RainCtlCommand::Info => {
            let info = make_request_or_start(config, InfoRequest, |()| {}).map_err(|err| {
                eprintln!("{err}");
            })?;
            println!("{info:#?}");
            Ok(())
        }
        RainCtlCommand::Shutdown => {
            make_request_or_start(config, ShutdownRequest, |()| {}).map_err(|err| {
                eprintln!("{err}");
            })?;
            println!("Server shutdown");
            Ok(())
        }
        RainCtlCommand::Config => {
            eprintln!("{config:#?}");
            Ok(())
        }
        RainCtlCommand::Inspect => {
            let response =
                make_request_or_start(config, InspectRequest, |()| {}).map_err(|err| {
                    eprintln!("{err}");
                })?;
            eprintln!("{response:#?}");
            Ok(())
        }
        RainCtlCommand::Clean => clean(config),
    }
}

fn run(config: &Config, target: String) -> Result<(), ()> {
    let root = rain_core::find_root_rain().ok_or(())?;
    let mut stack = Vec::new();
    let run_response = make_request_or_start(config, RunRequest { root, target }, |im| {
        match im {
            RunProgress::Print(s) => println!("\r{s:40}"),
            RunProgress::EnterCall(s) => {
                if !s.starts_with("internal.") {
                    stack.push(s);
                }
            }
            RunProgress::ExitCall(s) => {
                if !s.starts_with("internal.") {
                    stack.pop();
                }
            }
        }
        if let Some(last) = stack.last() {
            print!("\r[ ] {last:40}");
        } else {
            print!("\r[x] {:40}", "Done");
        }
        let _ = stdout().flush();
    })
    .map_err(|err| {
        eprintln!("{err}");
    })?;
    println!();
    let result = run_response.output;
    match result {
        Ok(s) => {
            println!("{s}");
            Ok(())
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
            Err(())
        }
    }
}

fn clean(config: &Config) -> Result<(), ()> {
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
        let resp = make_request_or_start(config, CleanRequest, |()| {}).map_err(|err| {
            eprintln!("{err}");
        })?;
        println!("Cleaned");
        for (p, s) in resp.0 {
            println!(
                "  {:8} {}",
                humansize::format_size(s, humansize::BINARY),
                p.display(),
            );
        }
    } else {
        println!("Did nothing");
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
    },
    Shutdown,
    /// View rain config
    Config,
    /// Inspect the rain cache
    Inspect,
    /// Clean the rain cache
    Clean,
}
