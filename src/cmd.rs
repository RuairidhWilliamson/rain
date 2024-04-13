mod clean;
mod config;
mod debug;
mod run;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};

/// Rain build system command line interface
#[derive(Parser)]
#[command(version, author)]
pub struct Cli {
    #[command(subcommand)]
    command: RainCommand,

    /// Root directory of the current project, defaults to the current working directory
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum RainCommand {
    /// Run a rain target
    Run(run::RunCommand),
    /// Configure rain
    Config {
        #[command(subcommand)]
        command: config::ConfigCommand,
    },
    /// Debug rain
    Debug {
        #[command(subcommand)]
        command: debug::DebugCommand,
    },
    /// Clean rain's cache directory
    Clean(clean::CleanCommand),
}

impl Cli {
    pub fn run(self) -> ExitCode {
        let workspace_root = self
            .root
            .unwrap_or_else(Self::find_workspace_root)
            .canonicalize()
            .unwrap();
        tracing::info!("Workspace root {workspace_root:?}");

        let config = match crate::config::load(&workspace_root).validate() {
            Ok(config) => Box::leak(Box::new(config)),
            Err(err) => {
                eprintln!("validate config error: {err:#}");
                return ExitCode::FAILURE;
            }
        };
        match self.command {
            RainCommand::Run(command) => command.run(&workspace_root, config),
            RainCommand::Config { command } => command.run(&workspace_root, config),
            RainCommand::Debug { command } => command.run(),
            RainCommand::Clean(command) => command.run(&workspace_root, config),
        }
    }

    fn find_workspace_root() -> PathBuf {
        let p = Path::new(".");
        p.to_path_buf()
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
