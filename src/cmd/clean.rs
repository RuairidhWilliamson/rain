use std::{path::Path, process::ExitCode};

use clap::Args;
use rain_lang::{config::global_config, path::Workspace};

#[derive(Args)]
pub struct CleanCommand {
    #[arg(long)]
    pub confirm: bool,
}

impl CleanCommand {
    pub fn run(self, _root_workspace: &Workspace) -> ExitCode {
        let global_cache_size =
            Self::calc_size(&global_config().cache_directory).expect("read cache directory");
        eprintln!("Will delete all files in:");
        eprintln!(
            "  {:.1} MiB {}",
            global_cache_size as f32 / 1024.0 / 1024.0,
            global_config().cache_directory.display()
        );
        if !self.confirm {
            match dialoguer::Confirm::new()
                .with_prompt("Do you want to delete these directories?")
                .interact()
            {
                Ok(true) => (),
                Ok(false) => return ExitCode::FAILURE,
                Err(err) => {
                    tracing::error!("Error {err:#}");
                    return ExitCode::FAILURE;
                }
            }
            tracing::info!("Clean confirmed");
        } else {
            tracing::info!("Clean confirm bypassed");
        }
        std::fs::remove_dir_all(&global_config().cache_directory).expect("remove cache directory");
        tracing::info!("Removed recursively {:?}", global_config().cache_directory);
        ExitCode::SUCCESS
    }

    fn calc_size(path: &Path) -> std::io::Result<u64> {
        let mut sum = 0;
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;
            sum += entry.metadata()?.len();
        }
        Ok(sum)
    }
}
