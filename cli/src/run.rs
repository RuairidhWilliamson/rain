use std::fmt::Write as _;

use crate::GlobalOptions;
use crate::remote::{
    client::{ClientMode, make_request_or_start},
    msg::run::{RunRequest, RunResponse},
};
use rain_core::{CoreError, config::Config};
use termcolor::WriteColor as _;

pub fn run(
    config: &Config,
    target: &str,
    args: Vec<String>,
    options: &GlobalOptions,
    mode: ClientMode,
) -> Result<(), ()> {
    let custom_config = options.parse_config()?;
    let root = options.resolve_entrypoint()?;
    let mut reporter = crate::reporter::new_reporter(options);
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
            verification: options.verification,
        },
        |progress| reporter.update(progress),
        mode,
    )
    .map_err(|err| {
        eprintln!("{err}");
    })?;
    handle_run_response(target, options, run_response)
}

fn handle_run_response(
    target: &str,
    options: &GlobalOptions,
    run_response: RunResponse,
) -> Result<(), ()> {
    let mut color_stderr = termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto);
    let RunResponse {
        output: result,
        mut deps,
        elapsed,
    } = run_response;
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
                color_stderr.reset().expect("write stderr");
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
                    if target.is_empty() {
                        eprintln!("no declaration specified, try one of:\n{suggestions}");
                    } else {
                        eprintln!(
                            "unknown declaration {unknown} in {target:?}, try one of:\n{prefix}\n{suggestions}"
                        );
                    }
                }
                CoreError::Other(s) => {
                    eprintln!("{s}");
                }
            }
            Err(())
        }
    }
}
