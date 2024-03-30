mod cmd;
mod config;
mod error_display;
mod stdlib;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cli = cmd::Cli::parse();
    cli.run()
}
