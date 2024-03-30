use std::{path::Path, process::ExitCode};

use clap::Subcommand;

use crate::config::{config_search_paths, Config};

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    Paths,
}

impl ConfigCommand {
    pub fn run(self, _workspace_root: &Path, config: &Config) -> ExitCode {
        match self {
            Self::Show => {
                println!("{}", toml::to_string_pretty(config).unwrap());
                ExitCode::SUCCESS
            }
            Self::Paths => {
                for p in config_search_paths(_workspace_root) {
                    println!("{}", p.display());
                }
                ExitCode::SUCCESS
            }
        }
    }
}
