use std::process::ExitCode;

use clap::Args;
use rain_lang::path::Workspace;

#[derive(Args)]
pub struct WatchCommand {
    target: Option<String>,
}

impl WatchCommand {
    pub fn run(self, _workspace: &Workspace) -> ExitCode {
        todo!()
    }
}
