use std::{path::Path, process::ExitCode};

use clap::Subcommand;
use rain_lang::config::global_config;

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Show the evaluated config
    Show,
    /// Show the paths where config files are searched for
    Paths,
}

impl ConfigCommand {
    pub fn run(self, workspace_root_directory: &Path) -> ExitCode {
        match self {
            Self::Show => {
                println!(
                    "{}",
                    toml::to_string_pretty(global_config()).expect("serialize toml")
                );
                ExitCode::SUCCESS
            }
            Self::Paths => {
                for p in rain_lang::config::config_search_paths(workspace_root_directory) {
                    println!("{}", p.display());
                }
                ExitCode::SUCCESS
            }
        }
    }
}
