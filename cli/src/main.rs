#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod exe;
mod remote;

use std::{
    borrow::Cow,
    ffi::OsStr,
    io::{Write as _, stderr, stdin},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use env_logger::Env;
use rain_core::{CoreError, config::Config};
use remote::{
    client::{ClientMode, make_request_or_start},
    msg::{
        clean::CleanRequest,
        info::InfoRequest,
        inspect::{InspectRequest, InspectResponse},
        prune::{PruneRequest, Pruned},
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
    ctrlc::set_handler(|| {
        println!("\nCTRL+C pressed");
        std::process::exit(1);
    })
    .expect("init signal handler");
    let cli = Cli::parse();
    let mode = ClientMode::BackgroundThread;
    match cli.command {
        RainCtlCommand::Check => run(
            config,
            String::from("check"),
            vec![],
            false,
            &cli.options,
            ReportMode::Short,
            mode,
        ),
        RainCtlCommand::Build => run(
            config,
            String::from("build"),
            vec![],
            false,
            &cli.options,
            ReportMode::Short,
            mode,
        ),
        RainCtlCommand::Run {
            resolve,
            report,
            target,
            args,
        } => run(config, target, args, resolve, &cli.options, report, mode),
        RainCtlCommand::Info => {
            let info =
                make_request_or_start(config, InfoRequest, |()| {}, mode).map_err(|err| {
                    eprintln!("{err}");
                })?;
            println!("{info:#?}");
            Ok(())
        }
        RainCtlCommand::Shutdown => {
            make_request_or_start(config, ShutdownRequest, |()| {}, mode).map_err(|err| {
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
            } = make_request_or_start(config, InspectRequest, |()| {}, mode).map_err(|err| {
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
                let path = config.base_generated_dir.join(line);
                println!("{}", path.display());
            }
            Ok(())
        }
        RainCtlCommand::Clean => clean(config, mode),
        RainCtlCommand::Prune => prune(config, mode),
    }
}

fn run(
    config: &Config,
    target: String,
    args: Vec<String>,
    resolve: bool,
    options: &GlobalOptions,
    reporting: ReportMode,
    mode: ClientMode,
) -> Result<(), ()> {
    let root = rain_core::find_main_rain()
        .ok_or(())
        .map_err(|()| eprintln!("no main.rain found"))?;
    let mut stack = Vec::new();
    let run_response = make_request_or_start(
        config,
        RunRequest {
            root,
            target,
            args,
            resolve,
            offline: options.offline,
            host_override: options.host.clone(),
        },
        |im| {
            if reporting != ReportMode::Short {
                return;
            }
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
                eprint!("\r[ ] {:120}", trunc_string(last, 120));
            }
            let _ = stderr().flush();
        },
        mode,
    )
    .map_err(|err| {
        eprintln!("{err}");
    })?;
    let RunResponse {
        output: result,
        elapsed,
    } = run_response;
    if reporting == ReportMode::Short {
        eprint!("\r[x] {:120}\r", "");
    }
    match result {
        Ok(s) => {
            eprintln!("✔  Success in {elapsed:.1?}");
            println!("{s}");
            Ok(())
        }
        Err(s) => {
            eprintln!("❗ Error in {elapsed:.1?}");
            match s {
                CoreError::LangError(owned_resolved_error) => {
                    let mut stderr =
                        termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto);
                    owned_resolved_error
                        .write_color(&mut stderr)
                        .expect("write stdout");
                }
                CoreError::UnknownDeclaration(suggestions) => {
                    eprintln!("unknown declaration, try one of {suggestions:?}");
                }
                CoreError::Other(s) => {
                    eprintln!("{s}");
                }
            }
            Err(())
        }
    }
}

fn clean(config: &Config, mode: ClientMode) -> Result<(), ()> {
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
        let resp = make_request_or_start(config, CleanRequest, |()| {}, mode).map_err(|err| {
            eprintln!("{err}");
        })?;
        if resp.0.is_empty() {
            println!("Nothing to clean");
        } else {
            println!("Cleaned");
            for (p, s) in resp.0 {
                println!(
                    "  {:8} {}",
                    humansize::format_size(s, humansize::BINARY),
                    p.display(),
                );
            }
        }
    } else {
        println!("Did nothing");
    }
    Ok(())
}

fn prune(config: &Config, mode: ClientMode) -> Result<(), ()> {
    let Pruned(size) =
        make_request_or_start(config, PruneRequest, |()| {}, mode).map_err(|err| {
            eprintln!("{err}");
        })?;
    println!(
        "Pruned {:8}",
        humansize::format_size(size, humansize::BINARY)
    );
    Ok(())
}

#[derive(Debug, Clone, Parser)]
struct GlobalOptions {
    /// Disable performing actions that require an internet connection and try to use cache more often
    #[arg(long, global = true, env = "RAIN_OFFLINE")]
    offline: bool,
    /// Override the host to a custom triple
    #[arg(long, global = true, env = "RAIN_HOST")]
    host: Option<String>,
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
    /// Get information about the running rain server process
    Info,
    /// Run checks
    /// Equivalent to `rain run check`
    Check,
    /// Build!
    /// Equivalent to `rain run build`
    Build,
    Run {
        /// Resolve returned file paths before printing them to stdout
        #[arg(long)]
        resolve: bool,
        /// The reporting mode to use
        #[arg(long, default_value = "short")]
        report: ReportMode,
        target: String,
        args: Vec<String>,
    },
    /// Stop the rain server process
    Shutdown,
    /// View rain config
    Config,
    /// Inspect the rain cache
    Inspect,
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

fn trunc_string(s: &str, limit: usize) -> Cow<'_, str> {
    if s.len() <= limit {
        return s.into();
    }
    (s[..limit - 3].to_owned() + "...").into()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
enum ReportMode {
    #[default]
    Short,
    None,
}
