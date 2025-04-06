#![allow(clippy::print_stderr, clippy::print_stdout)]

mod exe;
mod remote;

use std::{
    ffi::OsStr,
    io::{Write as _, stderr, stdin},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use env_logger::Env;
use rain_core::{CoreError, config::Config};
use remote::{
    client::make_request_or_start,
    msg::{
        clean::CleanRequest,
        info::InfoRequest,
        inspect::{InspectRequest, InspectResponse},
        run::{RunProgress, RunRequest, RunResponse},
        shutdown::ShutdownRequest,
    },
};

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
    rain_ctl_command(&config)
}

fn rain_ctl_command(config: &Config) -> Result<(), ()> {
    let cli = Cli::parse();
    match cli.command {
        RainCtlCommand::Check => run(config, String::from("check"), vec![], false),
        RainCtlCommand::Build => run(config, String::from("build"), vec![], false),
        RainCtlCommand::Run {
            target,
            resolve,
            args,
        } => run(config, target, args, resolve),
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
            eprintln!("Server shutdown");
            Ok(())
        }
        RainCtlCommand::Config => {
            eprintln!("{config:#?}");
            Ok(())
        }
        RainCtlCommand::Inspect => {
            let InspectResponse {
                cache_size,
                entries,
            } = make_request_or_start(config, InspectRequest, |()| {}).map_err(|err| {
                eprintln!("{err}");
            })?;
            eprintln!("Cache size is {cache_size}");
            for e in entries {
                eprintln!("{e}");
            }
            Ok(())
        }
        RainCtlCommand::Resolve { path } => {
            let lines: Box<dyn Iterator<Item = String>> = if let Some(p) = path {
                Box::new(std::iter::once(p))
            } else {
                Box::new(stdin().lines().map(|s| s.expect("read stdin")))
            };
            for line in lines {
                let (area, rest) = line.split_once('/').unwrap_or((&line, ""));
                let area = area
                    .strip_prefix('<')
                    .ok_or_else(|| eprintln!("missing <"))?
                    .strip_suffix('>')
                    .ok_or_else(|| eprintln!("missing >"))?;
                let path = config.base_generated_dir.join(area).join(rest);
                println!("{}", path.display());
            }
            Ok(())
        }
        RainCtlCommand::Clean => clean(config),
    }
}

fn run(config: &Config, target: String, args: Vec<String>, resolve: bool) -> Result<(), ()> {
    let root = rain_core::find_root_rain().ok_or(())?;
    let mut stack = Vec::new();
    let run_response = make_request_or_start(
        config,
        RunRequest {
            root,
            target,
            args,
            resolve,
        },
        |im| {
            match im {
                RunProgress::Print(s) => eprintln!("\r{s:120}"),
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
                eprint!("\r[ ] {last:120}");
            }
            let _ = stderr().flush();
        },
    )
    .map_err(|err| {
        eprintln!("{err}");
    })?;
    let RunResponse {
        output: result,
        elapsed,
    } = run_response;
    eprint!("\r[x] {:120}\r", "");
    match result {
        Ok(s) => {
            eprintln!("Done in {elapsed:.1?}");
            println!("{s}");
            Ok(())
        }
        Err(s) => {
            match s {
                CoreError::LangError(owned_resolved_error) => {
                    eprintln!("Error in {elapsed:.1?}");
                    let mut stderr =
                        termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto);
                    owned_resolved_error
                        .write_color(&mut stderr)
                        .expect("write stdout");
                }
                CoreError::Other(s) => {
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
    if inquire::Confirm::new("Delete all these directories recursively?")
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
    Check,
    Build,
    Run {
        target: String,
        /// Resolve returned file paths before printing them to stdout
        #[arg(long)]
        resolve: bool,
        args: Vec<String>,
    },
    Shutdown,
    /// View rain config
    Config,
    /// Inspect the rain cache
    Inspect,
    /// Resolve rain path to actual local path
    Resolve {
        path: Option<String>,
    },
    /// Clean the rain cache
    Clean,
}

#[test]
fn validate_cli() {
    <Cli as clap::CommandFactory>::command().debug_assert();
}
