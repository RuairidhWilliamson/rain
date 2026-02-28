use std::fmt::Write as _;
use std::io::{Write as _, stderr};

use crate::ReportMode;
use crate::remote::{
    client::{ClientMode, make_request_or_start},
    msg::run::{RunProgress, RunRequest, RunResponse},
};
use rain_core::{CoreError, config::Config};
use termcolor::WriteColor as _;

pub fn run(
    config: &Config,
    target: &str,
    args: Vec<String>,
    options: &crate::GlobalOptions,
    mode: ClientMode,
) -> Result<(), ()> {
    let custom_config = options.parse_config()?;
    let root = options.resolve_entrypoint()?;
    let mut stack = Vec::new();
    let run_response = make_request_or_start(
        config,
        RunRequest {
            root,
            target: target.to_owned(),
            args,
            resolve: options.resolve,
            offline: options.offline,
            seal: options.seal,
            host_override: options.host.clone(),
            custom_config,
        },
        |im| match options.report {
            ReportMode::Basic => {
                match im {
                    RunProgress::Print(s) => eprintln!("{s}"),
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
                    eprintln!("{last}");
                }
                let _ = stderr().flush();
            }
            ReportMode::Verbose => {
                match im {
                    RunProgress::Print(s) => eprintln!("{s}"),
                    RunProgress::EnterCall(s) => {
                        stack.push(s);
                    }
                    RunProgress::ExitCall(_) => {
                        stack.pop();
                    }
                }
                if let Some(last) = stack.last() {
                    eprintln!("{last}");
                }
                let _ = stderr().flush();
            }
            ReportMode::None => {}
        },
        mode,
    )
    .map_err(|err| {
        eprintln!("{err}");
    })?;
    handle_run_response(target, options, run_response)
}

fn handle_run_response(
    target: &str,
    options: &crate::GlobalOptions,
    run_response: RunResponse,
) -> Result<(), ()> {
    let mut color_stderr = termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto);
    let RunResponse {
        output: result,
        mut deps,
        elapsed,
    } = run_response;
    if options.report == ReportMode::Basic {
        eprint!("\r{:120}\r", "");
    }
    match result {
        Ok(s) => {
            eprintln!("✔  Success in {elapsed:.1?}");
            if options.deps {
                deps.sort_and_unique();
                eprintln!("{} Deps:", deps.len());
                for d in deps {
                    d.write_color(&mut color_stderr).expect("write stderr");
                    eprintln!();
                }
                color_stderr.reset().unwrap();
            }
            println!("{s}");
            Ok(())
        }
        Err(s) => {
            eprintln!("❗ Error in {elapsed:.1?}");
            match s {
                CoreError::LangError(owned_resolved_error) => {
                    owned_resolved_error
                        .write_color(&mut color_stderr)
                        .expect("write stderr");
                }
                CoreError::UnknownDeclaration {
                    prefix,
                    unknown,
                    suggestions,
                } => {
                    let suggestions: String =
                        suggestions.into_iter().fold(String::new(), |mut acc, s| {
                            let _ = writeln!(acc, "\t{s}");
                            acc
                        });
                    eprintln!(
                        "unknown declaration {unknown} in {target:?}, try one of:\n{prefix}\n{suggestions}"
                    );
                }
                CoreError::Other(s) => {
                    eprintln!("{s}");
                }
            }
            Err(())
        }
    }
}
