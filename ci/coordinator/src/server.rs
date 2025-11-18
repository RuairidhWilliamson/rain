use std::{convert::Infallible, sync::Arc};

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http::{Request, Response, request::Parts};
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use log::{error, info};
use rain_lang::afs::{dir::Dir, file::File};
use rain_lang::afs::{entry::FSEntry, entry::FSEntryTrait as _, path::SealedFilePath};
use rain_lang::driver::{DriverTrait as _, FSTrait as _};

use crate::runner::RunComplete;
use crate::{github::InstallationClient as _, runner::Runner};

pub struct Server<GH: crate::github::Client, ST: crate::storage::StorageTrait> {
    pub target_url: url::Url,
    pub github_webhook_secret: String,
    pub runner: Runner,
    pub github_client: GH,
    pub storage: ST,
    pub tx: tokio::sync::mpsc::Sender<crate::github::model::CheckSuiteEvent>,
}

impl<GH: crate::github::Client, ST: crate::storage::StorageTrait> Server<GH, ST> {
    pub async fn handle_request(
        self: Arc<Self>,
        request: Request<Incoming>,
    ) -> Result<Response<String>, Infallible> {
        match request.uri().path() {
            "/webhook/github" => {
                match self.handle_github_event(request).await {
                    Ok(()) => (),
                    Err(err) => {
                        error!("{err}");
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

        let check_suite_event: crate::github::model::CheckSuiteEvent =
            serde_json::from_slice(&body[..])?;

        self.tx.send(check_suite_event).await?;
        Ok(())
    }

    #[expect(clippy::unwrap_used)]
    pub async fn handle_check_suite_event(
        self: Arc<Self>,
        check_suite_event: crate::github::model::CheckSuiteEvent,
    ) -> std::result::Result<(), anyhow::Error> {
        if !matches!(
            check_suite_event.check_suite.status,
            Some(crate::github::model::Status::Queued)
        ) {
            info!(
                "skipping check suite event {:?}",
                check_suite_event.check_suite.status
            );
            return Ok(());
        }

        let installation_client = Arc::new(
            self.github_client
                .auth_installation(check_suite_event.installation.id)?,
        );
        info!("received {check_suite_event:?}");

        let owner = check_suite_event.repository.owner.login;
        let repo = check_suite_event.repository.name;
        let head_sha = check_suite_event.check_suite.head_sha;

        let check_run = installation_client
            .create_check_run(
                &owner,
                &repo,
                crate::github::model::CreateCheckRun {
                    name: String::from("rainci"),
                    head_sha: head_sha.clone(),
                    status: crate::github::model::Status::Queued,
                    details_url: Some(self.target_url.to_string()),
                    output: None,
                },
            )
            .await
            .context("create check run")?;
        let start = chrono::Utc::now();
        let run_id = self
            .storage
            .create_run(rain_ci_common::Run {
                source: rain_ci_common::RunSource::Github,
                created_at: start,
                commit: head_sha.clone(),
                repository: rain_ci_common::Repository {
                    owner: owner.clone(),
                    name: repo.clone(),
                },
                dequeued_at: None,
                finished: None,
            })
            .await
            .context("storage create run")?;

        info!("created check run {run_id}");

        self.storage
            .dequeued_run(&run_id)
            .await
            .context("storage dequeue run")?;

        installation_client
            .update_check_run(
                &owner,
                &repo,
                check_run.id,
                crate::github::model::PatchCheckRun {
                    status: Some(crate::github::model::Status::InProgress),
                    ..Default::default()
                },
            )
            .await
            .context("update check run")?;

        let handle = {
            let owner = owner.clone();
            let repo = repo.clone();
            let server = Arc::clone(&self);
            let installation_client = Arc::clone(&installation_client);
            tokio::task::spawn_blocking(move || {
                let download = installation_client
                    .download_repo_tar(&owner, &repo, &head_sha)
                    .context("download repo")?;
                let download_dir_name = format!("{owner}-{repo}-{head_sha}");
                let driver = rain_core::driver::DriverImpl::new(rain_core::config::Config::new());
                let download_area = driver.create_area(&[]).unwrap();
                let download_entry =
                    FSEntry::new(download_area, SealedFilePath::new("/download").unwrap());
                std::fs::write(driver.resolve_fs_entry(&download_entry), download).unwrap();
                let download = File::new_checked(&driver, download_entry).unwrap();
                let area = driver.extract_tar_gz(&download).unwrap();
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
                installation_client
                    .smudge_git_lfs(&owner, &repo, lfs_entries)
                    .context("smudge git lfs")?;
                let area = driver.create_area(&[root.inner()]).unwrap();
                let run_complete = server.runner.run(&driver, area);
                Ok(run_complete)
            })
        };

        let result: Result<Result<RunComplete>, _> = handle.await;

        let (status, conclusion, output) = match result {
            Ok(Ok(RunComplete {
                success: true,
                output,
            })) => (
                rain_ci_common::RunStatus::Success,
                crate::github::model::CheckRunConclusion::Success,
                output,
            ),
            Ok(Ok(RunComplete {
                success: false,
                output,
            })) => (
                rain_ci_common::RunStatus::Failure,
                crate::github::model::CheckRunConclusion::Failure,
                output,
            ),
            Ok(Err(err)) => {
                log::error!("runner error: {err:?}");
                (
                    rain_ci_common::RunStatus::Failure,
                    crate::github::model::CheckRunConclusion::Failure,
                    String::default(),
                )
            }
            Err(err) => {
                log::error!("runner panicked: {err:?}");
                (
                    rain_ci_common::RunStatus::Failure,
                    crate::github::model::CheckRunConclusion::Failure,
                    String::default(),
                )
            }
        };

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
                crate::github::model::PatchCheckRun {
                    status: Some(crate::github::model::Status::Completed),
                    conclusion: Some(conclusion),
                    output: Some(crate::github::model::CheckRunOutput {
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
