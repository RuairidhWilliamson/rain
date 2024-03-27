mod debug;
mod run;

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

/// Rain build system command line interface
#[derive(Parser)]
#[command(version, author)]
pub struct Cli {
    #[command(subcommand)]
    command: RainCommand,

    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum RainCommand {
    Run(run::RunCommand),
    Debug {
        #[command(subcommand)]
        command: debug::DebugCommand,
    },
}

impl Cli {
    pub fn run(self) -> ExitCode {
        match self.command {
            RainCommand::Run(command) => command.run(),
            RainCommand::Debug { command } => command.run(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        super::Cli::command().debug_assert()
    }
}
