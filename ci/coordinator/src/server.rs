use std::{convert::Infallible, sync::Arc};

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http::{Request, Response, request::Parts};
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use log::{error, info};
use rain_ci_common::RunStatus;
use rain_ci_common::github::InstallationClient as _;
use rain_ci_common::github::model::CheckRunConclusion;
use rain_lang::afs::{dir::Dir, file::File};
use rain_lang::afs::{entry::FSEntry, entry::FSEntryTrait as _, path::SealedFilePath};
use rain_lang::driver::{DriverTrait as _, FSTrait as _};
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use crate::RunRequest;
use crate::runner::RunComplete;
use crate::runner::Runner;

pub struct Server<GH: rain_ci_common::github::Client, ST: crate::storage::StorageTrait> {
    pub target_url: url::Url,
    pub github_webhook_secret: String,
    pub runner: Runner,
    pub github_client: GH,
    pub storage: ST,
    pub tx: tokio::sync::mpsc::Sender<RunRequest>,
}

impl<GH: rain_ci_common::github::Client, ST: crate::storage::StorageTrait> Server<GH, ST> {
    pub fn start_server_run_request_worker(self: &Arc<Self>, mut rx: Receiver<RunRequest>) {
        let server = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let Some(check_suite_event) = rx.recv().await else {
                    error!("server recv channel closed");
                    return;
                };
                if let Err(err) = Arc::clone(&server)
                    .handle_run_request(check_suite_event)
                    .await
                {
                    error!("handle check suite event: {err}");
                }
            }
        });
    }

    pub async fn handle_request(
        self: Arc<Self>,
        request: Request<Incoming>,
    ) -> Result<Response<String>, Infallible> {
        match request.uri().path() {
            "/webhook/github" => {
                match self.handle_github_event(request).await {
                    Ok(()) => (),
                    Err(err) => {
                        error!("{err:#}");
                    }
                }
                Ok(Response::builder()
                    .status(http::status::StatusCode::OK)
                    .body(String::default())
                    .expect("build response"))
            }
            _ => Ok(Response::builder()
                .status(http::status::StatusCode::NOT_FOUND)
                .body(String::default())
                .expect("buid response")),
        }
    }

    async fn handle_github_event(self: Arc<Self>, request: Request<Incoming>) -> Result<()> {
        let headers = request.headers();
        let content_type = headers
            .get("Content-Type")
            .context("missing content type")?;
        if content_type.as_bytes() != b"application/json" {
            return Err(anyhow!("unexpected content type"));
        }
        let user_agent = headers.get("User-Agent").context("missing user agent")?;
        if !user_agent.as_bytes().starts_with(b"GitHub-Hookshot/") {
            return Err(anyhow!("unexpected user agent"));
        }
        let (parts, body) = request.into_parts();
        let body = body.collect().await?.to_bytes();
        self.verify_webhook_signature(&parts, &body[..])?;

        let event_kind = parts
            .headers
            .get("x-github-event")
            .context("missing github event kind")?
            .to_str()?;

        if event_kind != "check_suite" {
            return Err(anyhow!("unexpected event kind: {event_kind:?}"));
        }

        let check_suite_event: rain_ci_common::github::model::CheckSuiteEvent =
            serde_json::from_slice(&body[..])?;

        if !matches!(
            check_suite_event.check_suite.status,
            Some(rain_ci_common::github::model::Status::Queued)
        ) {
            info!(
                "skipping check suite event {:?}",
                check_suite_event.check_suite.status
            );
            return Ok(());
        }

        info!("received {check_suite_event:?}");

        let owner = check_suite_event.repository.owner.login;
        let repo = check_suite_event.repository.name;
        let head_sha = check_suite_event.check_suite.head_sha;

        let start = chrono::Utc::now();
        let repo_host = rain_ci_common::RepoHost::Github;
        let repo_id = self
            .storage
            .create_or_get_repo(&repo_host, &owner, &repo)
            .await
            .context("resolve repo id")?;
        let run_id = self
            .storage
            .create_run(rain_ci_common::Run {
                created_at: start,
                commit: head_sha.clone(),
                repository: rain_ci_common::Repository {
                    id: repo_id,
                    host: repo_host,
                    owner: owner.clone(),
                    name: repo.clone(),
                },
                dequeued_at: None,
                finished: None,
                target: String::from("ci"),
                rain_version: None,
            })
            .await
            .context("storage create run")?;

        self.tx.send(RunRequest { run_id }).await?;
        Ok(())
    }

    #[expect(clippy::unwrap_used)]
    pub async fn handle_run_request(
        self: Arc<Self>,
        run_request: RunRequest,
    ) -> Result<(), anyhow::Error> {
        let installations = self.github_client.app_installations().await?;
        // FIXME: Getting the first installation is a bad assumption
        let installation = installations.first().unwrap();
        let installation_client = Arc::new(
            self.github_client
                .auth_installation(installation.id)
                .await?,
        );
        let run_id = run_request.run_id;
        let start = chrono::Utc::now();
        let run = self.storage.get_run(&run_id).await?;

        let owner = run.repository.owner;
        let repo = run.repository.name;
        let head_sha = run.commit.clone();

        let check_run = installation_client
            .create_check_run(
                &owner,
                &repo,
                rain_ci_common::github::model::CreateCheckRun {
                    name: String::from("rainci"),
                    head_sha: head_sha.clone(),
                    status: rain_ci_common::github::model::Status::Queued,
                    details_url: Some(self.target_url.to_string()),
                    output: None,
                },
            )
            .await
            .context("create check run")?;

        self.storage
            .dequeued_run(&run_id)
            .await
            .context("storage dequeue run")?;

        installation_client
            .update_check_run(
                &owner,
                &repo,
                check_run.id,
                rain_ci_common::github::model::PatchCheckRun {
                    status: Some(rain_ci_common::github::model::Status::InProgress),
                    ..Default::default()
                },
            )
            .await
            .context("update check run")?;

        log::info!("Preparing run");
        let result_handle = self
            .download_and_run(&installation_client, &owner, &repo, head_sha, run.target)
            .await;

        let (status, conclusion, output) = resolve_error(result_handle).await;

        let finished_at = Utc::now();
        let execution_time = finished_at - start;
        self.storage
            .finished_run(
                &run_id,
                rain_ci_common::FinishedRun {
                    finished_at,
                    status,
                    execution_time,
                    output: output.clone(),
                },
            )
            .await
            .context("storage finished run")?;
        installation_client
            .update_check_run(
                &owner,
                &repo,
                check_run.id,
                rain_ci_common::github::model::PatchCheckRun {
                    status: Some(rain_ci_common::github::model::Status::Completed),
                    conclusion: Some(conclusion),
                    output: Some(rain_ci_common::github::model::CheckRunOutput {
                        title: String::from("rain run"),
                        summary: String::from("rain run complete"),
                        text: output.replace(' ', "&nbsp;"),
                    }),
                    ..Default::default()
                },
            )
            .await
            .context("update check run")?;

        self.runner.prune();

        Ok(())
    }

    async fn download_and_run(
        self: &Arc<Self>,
        installation_client: &Arc<impl rain_ci_common::github::InstallationClient>,
        owner: &str,
        repo: &str,
        head_sha: String,
        target: String,
    ) -> Result<JoinHandle<Result<RunComplete, anyhow::Error>>, anyhow::Error> {
        let server = Arc::clone(self);
        let installation_client = Arc::clone(installation_client);
        let download = installation_client
            .download_repo_tar(owner, repo, &head_sha)
            .await
            .context("download repo")?;
        #[expect(clippy::unwrap_used)]
        let (root, lfs_entries) = tokio::task::spawn_blocking(move || {
            let driver = rain_core::driver::DriverImpl::new(rain_core::config::Config::new());
            let download_area = driver.create_area(&[], true).unwrap();
            let download_entry =
                FSEntry::new(download_area, SealedFilePath::new("/download").unwrap());
            std::fs::write(driver.resolve_fs_entry(&download_entry), download).unwrap();
            let download = File::new_checked(&driver, download_entry).unwrap();
            let raw_tar = driver.extract_gzip(&download, "extract_temp.tar").unwrap();
            let area = driver.extract_tar(&raw_tar).unwrap();
            let mut ls =
                std::fs::read_dir(driver.resolve_fs_entry(Dir::root(area.clone()).inner()))
                    .unwrap();
            let entry = ls.next().unwrap().unwrap();
            let download_dir_name = entry.file_name().into_string().unwrap();
            let download_dir_entry =
                FSEntry::new(area, SealedFilePath::new(&download_dir_name).unwrap());
            let root = Dir::new_checked(&driver, download_dir_entry).unwrap();
            let lfs_entries: Vec<_> = driver
                .glob(&root, "**/*")
                .unwrap()
                .into_iter()
                .filter_map(|entry| {
                    let path = driver.resolve_fs_entry(entry.inner());
                    let lfs_object = git_lfs_rs::object::Object::from_path(&path).ok()?;
                    Some((path, lfs_object))
                })
                .collect();
            (root, lfs_entries)
        })
        .await?;
        installation_client
            .smudge_git_lfs(owner, repo, lfs_entries)
            .await
            .context("smudge git lfs")?;
        log::info!("Prepare run complete");
        #[expect(clippy::unwrap_used)]
        Ok(tokio::task::spawn_blocking(move || {
            let driver = rain_core::driver::DriverImpl::new(rain_core::config::Config::new());
            let area = driver
                .create_overlay_area(std::iter::once(root.inner()), true, true)
                .unwrap();
            let run_complete = server.runner.run(&driver, area, &target);
            Ok(run_complete)
        }))
    }

    fn verify_webhook_signature(&self, request: &Parts, body: &[u8]) -> Result<()> {
        let signature = request
            .headers
            .get("x-hub-signature-256")
            .context("signature not present")?
            .to_str()?;
        let (algo, sig_hex) = signature
            .split_once('=')
            .context("header does not contain =")?;
        if algo != "sha256" {
            return Err(anyhow!("unknown algorithm"));
        }
        let sig = hex::decode(sig_hex).context("decode signature hex")?;
        let key = ring::hmac::Key::new(
            ring::hmac::HMAC_SHA256,
            self.github_webhook_secret.as_bytes(),
        );
        ring::hmac::verify(&key, body, &sig).context("verify signature")?;
        Ok(())
    }
}

async fn resolve_error(
    result_handle: Result<JoinHandle<Result<RunComplete>>>,
) -> (RunStatus, CheckRunConclusion, String) {
    match result_handle {
        Ok(handle) => {
            let result: Result<Result<RunComplete>, _> = handle.await;
            match result {
                Ok(Ok(RunComplete {
                    success: true,
                    output,
                })) => (RunStatus::Success, CheckRunConclusion::Success, output),
                Ok(Ok(RunComplete {
                    success: false,
                    output,
                })) => (RunStatus::Failure, CheckRunConclusion::Failure, output),
                Ok(Err(err)) => {
                    log::error!("runner error: {err:?}");
                    (
                        RunStatus::Failure,
                        CheckRunConclusion::Failure,
                        String::default(),
                    )
                }
                Err(err) => {
                    log::error!("runner panicked: {err:?}");
                    (
                        RunStatus::Failure,
                        CheckRunConclusion::Failure,
                        String::default(),
                    )
                }
            }
        }
        Err(err) => {
            log::error!("runner download error: {err:?}");
            (
                RunStatus::Failure,
                CheckRunConclusion::Failure,
                String::default(),
            )
        }
    }
}
