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

    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum RainCommand {
    Run(run::RunCommand),
    Config {
        #[command(subcommand)]
        command: config::ConfigCommand,
    },
    Debug {
        #[command(subcommand)]
        command: debug::DebugCommand,
    },
}

impl Cli {
    pub fn run(self) -> ExitCode {
        let workspace_root = self
            .root
            .unwrap_or_else(Self::find_workspace_root)
            .canonicalize()
            .unwrap();
        tracing::info!("Workspace root {workspace_root:?}");
        let config = Box::leak(Box::new(crate::config::load(&workspace_root)));
        match self.command {
            RainCommand::Run(command) => command.run(&workspace_root, config),
            RainCommand::Config { command } => command.run(&workspace_root, config),
            RainCommand::Debug { command } => command.run(),
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
