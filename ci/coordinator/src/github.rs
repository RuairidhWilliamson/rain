pub mod implementation;
pub mod model;

use std::path::PathBuf;

use anyhow::Result;
use git_lfs_rs::object::Object;

pub trait Client: Send + Sync {
    fn auth_installation(
        &self,
        installation_id: model::InstallationId,
    ) -> Result<impl InstallationClient>;
}

pub trait InstallationClient: Send + Sync {
    fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run: model::CreateCheckRun,
    ) -> Result<model::CheckRun>;

    fn update_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: u64,
        check_run: model::PatchCheckRun,
    ) -> Result<model::CheckRun>;

    fn download_repo_tar(&self, owner: &str, repo: &str, git_ref: &str) -> Result<Vec<u8>>;

    fn smudge_git_lfs(
        &self,
        owner: &str,
        repo: &str,
        entries: Vec<(PathBuf, Object)>,
    ) -> Result<()>;
}
