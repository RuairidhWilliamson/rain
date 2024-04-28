mod cmd;
mod error_display;
mod stdlib;

use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("RAIN_LOG"))
        .init();

    let cli = cmd::Cli::parse();
    cli.run()
}
