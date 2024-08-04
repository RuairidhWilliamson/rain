mod clean;
mod config;
mod debug;
mod run;
mod watch;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use rain_lang::{config::set_global_config, path::Workspace};

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
    /// Keep running a rain target when a file system change is detected
    Watch(watch::WatchCommand),
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
        let root_workspace_directory = self
            .root
            .or_else(Self::find_root_workspace)
            .expect("could not find root workspace");

        match rain_lang::config::load(&root_workspace_directory).validate() {
            Ok(config) => set_global_config(config),
            Err(err) => {
                eprintln!("validate config error: {err:#}");
                return ExitCode::FAILURE;
            }
        };

        let root_workspace = Workspace::Local(root_workspace_directory.clone());
        tracing::info!("Workspace root {root_workspace:?}");

        match self.command {
            RainCommand::Run(command) => command.run(&root_workspace),
            RainCommand::Watch(command) => command.run(&root_workspace),
            RainCommand::Config { command } => command.run(&root_workspace_directory),
            RainCommand::Debug { command } => command.run(),
            RainCommand::Clean(command) => command.run(&root_workspace),
        }
    }

    pub fn find_root_workspace() -> Option<PathBuf> {
        let path = Path::new(".").canonicalize().unwrap();
        let mut p = path.as_path();
        loop {
            let manifest_path = p.join("rain.toml");
            if manifest_path.try_exists().unwrap() {
                return Some(p.to_path_buf());
            }
            p = p.parent()?;
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
