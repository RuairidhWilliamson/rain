pub mod implementation;
pub mod model;

use std::{future::Future, path::PathBuf};

use anyhow::Result;
use git_lfs_rs::object::Object;

pub trait Client: Send + Sync + 'static {
    fn auth_installation(
        &self,
        installation_id: model::InstallationId,
    ) -> impl Future<Output = Result<impl InstallationClient>> + Send;
    fn app_installations(&self) -> impl Future<Output = Result<Vec<model::Installation>>> + Send;
}

pub trait InstallationClient: Send + Sync + 'static {
    fn get_repo(
        &self,
        owner: &str,
        repo: &str,
    ) -> impl Future<Output = Result<model::Repository>> + Send;
    fn get_commit(
        &self,
        owner: &str,
        repo: &str,
        r#ref: &str,
    ) -> impl Future<Output = Result<model::Commit>> + Send;
    fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run: model::CreateCheckRun,
    ) -> impl Future<Output = Result<model::CheckRun>> + Send;
    fn update_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: u64,
        check_run: model::PatchCheckRun,
    ) -> impl Future<Output = Result<model::CheckRun>> + Send;
    fn download_repo_tar(
        &self,
        owner: &str,
        repo: &str,
        git_ref: &str,
    ) -> impl Future<Output = Result<Vec<u8>>> + Send;
    fn smudge_git_lfs(
        &self,
        owner: &str,
        repo: &str,
        entries: Vec<(PathBuf, Object)>,
    ) -> impl Future<Output = Result<()>> + Send;
}
