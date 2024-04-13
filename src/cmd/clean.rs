use std::{path::Path, process::ExitCode};

use clap::Args;

#[derive(Args)]
pub struct CleanCommand {
    #[arg(long)]
    pub confirm: bool,
}

impl CleanCommand {
    pub fn run(self, _workspace_root: &Path, config: &'static crate::config::Config) -> ExitCode {
        let global_cache_size = Self::calc_size(&config.cache_directory).unwrap();
        eprintln!("Will delete all files in:");
        eprintln!(
            "  {:.1} MiB {}",
            global_cache_size as f32 / 1024.0 / 1024.0,
            config.cache_directory.display()
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
        std::fs::remove_dir_all(&config.cache_directory).unwrap();
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
