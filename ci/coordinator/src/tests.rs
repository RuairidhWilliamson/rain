use anyhow::{Context as _, Result};
use chrono::Utc;
use rain_ci_common::{Run, RunId, github::model::InstallationId};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{RunRequest, runner::Runner, server::Server, storage::StorageTrait as _};

#[derive(Default)]
struct TestGithubClient {
    check_runs: Arc<Mutex<Vec<rain_ci_common::github::model::CheckRun>>>,
}

impl rain_ci_common::github::Client for TestGithubClient {
    async fn auth_installation(
        &self,
        _installation_id: rain_ci_common::github::model::InstallationId,
    ) -> Result<impl rain_ci_common::github::InstallationClient> {
        Ok(TestGithubInstallationClient {
            check_runs: Arc::clone(&self.check_runs),
        })
    }

    async fn app_installations(&self) -> Result<Vec<rain_ci_common::github::model::Installation>> {
        Ok(vec![rain_ci_common::github::model::Installation {
            id: InstallationId(0),
        }])
    }
}

struct TestGithubInstallationClient {
    check_runs: Arc<Mutex<Vec<rain_ci_common::github::model::CheckRun>>>,
}

impl rain_ci_common::github::InstallationClient for TestGithubInstallationClient {
    async fn get_commit(
        &self,
        _owner: &str,
        _repo: &str,
        _ref: &str,
    ) -> Result<rain_ci_common::github::model::Commit> {
        unimplemented!()
    }

    async fn get_repo(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> Result<rain_ci_common::github::model::Repository> {
        unimplemented!()
    }

    async fn create_check_run(
        &self,
        _owner: &str,
        _repo: &str,
        check_run: rain_ci_common::github::model::CreateCheckRun,
    ) -> Result<rain_ci_common::github::model::CheckRun> {
        let mut check_runs = self.check_runs.lock().await;
        let run = rain_ci_common::github::model::CheckRun {
            id: check_runs.len() as u64,
            name: check_run.name,
            head_sha: check_run.head_sha,
            status: check_run.status,
            conclusion: None,
        };
        check_runs.push(run.clone());
        Ok(run)
    }

    async fn update_check_run(
        &self,
        _owner: &str,
        _repo: &str,
        check_run_id: u64,
        patch: rain_ci_common::github::model::PatchCheckRun,
    ) -> Result<rain_ci_common::github::model::CheckRun> {
        let mut check_runs = self.check_runs.lock().await;
        let check_run = check_runs
            .get_mut(usize::try_from(check_run_id).unwrap())
            .context("check run does not exist")?;
        let new_check_run = rain_ci_common::github::model::CheckRun {
            id: check_run_id,
            name: patch.name.unwrap_or_else(|| check_run.name.clone()),
            head_sha: check_run.head_sha.clone(),
            status: patch.status.unwrap_or(check_run.status),
            conclusion: patch.conclusion.or(check_run.conclusion),
        };
        *check_run = new_check_run.clone();
        Ok(new_check_run)
    }

    async fn download_repo_tar(&self, owner: &str, repo: &str, git_ref: &str) -> Result<Vec<u8>> {
        assert_eq!(owner, "alice");
        assert_eq!(repo, "test");
        assert_eq!(git_ref, "abcd");
        Ok(include_bytes!("../test.tar.gz").to_vec())
    }

    async fn smudge_git_lfs(
        &self,
        _owner: &str,
        _repo: &str,
        _entries: Vec<(std::path::PathBuf, git_lfs_rs::object::Object)>,
    ) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
#[test_log::test]
async fn github_check_run() {
    let (tx, _) = tokio::sync::mpsc::channel(10);
    let server = Arc::new(Server {
        target_url: url::Url::parse("https://example.net").unwrap(),
        github_webhook_secret: String::new(),
        runner: Runner::new(true),
        github_client: TestGithubClient::default(),
        storage: crate::storage::test::Storage::default(),
        tx,
    });
    let repo_host = rain_ci_common::RepoHost::Github;
    let repo_owner = String::from("alice");
    let repo_name = String::from("test");
    let repo_id = server
        .storage
        .create_or_get_repo(&repo_host, &repo_owner, &repo_name)
        .await
        .unwrap();
    server
        .storage
        .create_run(Run {
            repository: rain_ci_common::Repository {
                id: repo_id,
                host: repo_host,
                owner: repo_owner,
                name: repo_name,
            },
            target: String::from("ci"),
            commit: String::from("abcd"),
            created_at: Utc::now(),
            dequeued_at: None,
            rain_version: None,
            finished: None,
        })
        .await
        .unwrap();
    Server::handle_run_request(Arc::clone(&server), RunRequest { run_id: RunId(0) })
        .await
        .unwrap();
    let check_runs = server.github_client.check_runs.lock().await;
    let check_run = check_runs.first().unwrap();
    assert_eq!(check_run.head_sha, "abcd");
    assert_eq!(
        check_run.status,
        rain_ci_common::github::model::Status::Completed
    );
}
