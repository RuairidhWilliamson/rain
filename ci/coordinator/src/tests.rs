use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result};

use crate::{runner::Runner, server::Server};

#[derive(Default)]
struct TestGithubClient {
    check_runs: Arc<Mutex<Vec<crate::github::model::CheckRun>>>,
}

impl crate::github::Client for TestGithubClient {
    fn auth_installation(
        &self,
        _installation_id: crate::github::model::InstallationId,
    ) -> Result<impl crate::github::InstallationClient> {
        Ok(TestGithubInstallationClient {
            check_runs: Arc::clone(&self.check_runs),
        })
    }
}

struct TestGithubInstallationClient {
    check_runs: Arc<Mutex<Vec<crate::github::model::CheckRun>>>,
}

impl crate::github::InstallationClient for TestGithubInstallationClient {
    async fn create_check_run(
        &self,
        _owner: &str,
        _repo: &str,
        check_run: crate::github::model::CreateCheckRun,
    ) -> Result<crate::github::model::CheckRun> {
        let mut check_runs = self.check_runs.lock().unwrap();
        let run = crate::github::model::CheckRun {
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
        patch: crate::github::model::PatchCheckRun,
    ) -> Result<crate::github::model::CheckRun> {
        let mut check_runs = self.check_runs.lock().unwrap();
        let check_run = check_runs
            .get_mut(usize::try_from(check_run_id).unwrap())
            .context("check run does not exist")?;
        let new_check_run = crate::github::model::CheckRun {
            id: check_run_id,
            name: patch.name.unwrap_or_else(|| check_run.name.clone()),
            head_sha: check_run.head_sha.clone(),
            status: patch.status.unwrap_or(check_run.status),
            conclusion: patch.conclusion.or(check_run.conclusion),
        };
        *check_run = new_check_run.clone();
        Ok(new_check_run)
    }

    fn download_repo_tar(&self, owner: &str, repo: &str, git_ref: &str) -> Result<Vec<u8>> {
        assert_eq!(owner, "alice");
        assert_eq!(repo, "test");
        assert_eq!(git_ref, "abcd");
        Ok(include_bytes!("../test.tar.gz").to_vec())
    }

    fn smudge_git_lfs(
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
    let server = Arc::new(Server {
        target_url: url::Url::parse("https://example.net").unwrap(),
        github_webhook_secret: String::new(),
        runner: Runner::new(true),
        github_client: TestGithubClient::default(),
        storage: crate::storage::test::Storage::default(),
    });
    let user = crate::github::model::User {
        id: 1,
        login: String::from("alice"),
        name: None,
        email: None,
    };
    Server::handle_check_suite_event(
        server.clone(),
        crate::github::model::CheckSuiteEvent {
            sender: user.clone(),
            repository: crate::github::model::Repository {
                name: String::from("test"),
                owner: user,
                default_branch: String::from("main"),
            },
            installation: crate::github::model::SimpleInstallation {
                id: crate::github::model::InstallationId(0),
                node_id: String::from("unknown"),
            },
            action: crate::github::model::Action::Requested,
            check_suite: crate::github::model::CheckSuite {
                id: 0,
                created_at: String::default(),
                head_sha: String::from("abcd"),
                head_branch: None,
                status: Some(crate::github::model::Status::Queued),
            },
        },
    )
    .await
    .unwrap();
    let check_runs = server.github_client.check_runs.lock().unwrap();
    let check_run = check_runs.first().unwrap();
    assert_eq!(check_run.head_sha, "abcd");
    assert_eq!(check_run.status, crate::github::model::Status::Completed);
}
