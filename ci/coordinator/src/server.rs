use std::{convert::Infallible, sync::Arc};

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http::{Request, Response, request::Parts};
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use log::{error, info};
use rain_ci_common::db::repository::Repository;
use rain_ci_common::db::repository_host::{RepoHostKind, RepositoryHost, RepositoryHostId};
use rain_ci_common::db::run::{FinishedRun, Run, RunStatus};
use rain_ci_common::db::{Db, Resource as _, WithId};
use rain_ci_common::github::implementation::{AppAuth, AppClient};
use rain_ci_common::github::model::{AppId, CheckRunConclusion};
use rain_ci_common::github::{Client as _, InstallationClient as _};
use rain_lang::afs::File;
use rain_lang::afs::area::FSArea;
use rain_lang::afs::dir::Dir;
use rain_lang::afs::generated::dir::GeneratedDir;
use rain_lang::afs::generated::entry::GeneratedFSEntry;
use rain_lang::afs::generated::file::GeneratedFile;
use rain_lang::afs::path::SealedFilePath;
use rain_lang::driver::{CreateAreaOptions, DriverTrait as _, FSTrait as _};
use secrecy::ExposeSecret as _;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use crate::RunRequest;
use crate::runner::RunComplete;
use crate::runner::Runner;

pub struct Server {
    pub target_url: url::Url,
    pub runner: Runner,
    pub db: Db,
    pub tx: tokio::sync::mpsc::Sender<RunRequest>,
}

impl Server {
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
        let Some(rest) = request.uri().path().strip_prefix("/webhook/") else {
            return Ok(Response::builder()
                .status(http::status::StatusCode::NOT_FOUND)
                .body(String::default())
                .expect("buid response"));
        };
        let Ok(repo_host_id) = rest.parse::<i64>() else {
            return Ok(Response::builder()
                .status(http::status::StatusCode::NOT_FOUND)
                .body(String::default())
                .expect("buid response"));
        };
        match self
            .handle_webhook(RepositoryHostId(repo_host_id), request)
            .await
        {
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

    async fn handle_webhook(
        self: Arc<Self>,
        repo_host_id: RepositoryHostId,
        request: Request<Incoming>,
    ) -> Result<()> {
        let repository_host = RepositoryHost::get(&self.db, repo_host_id).await?.resource;

        match repository_host.kind {
            RepoHostKind::Github => {}
            RepoHostKind::Gitlab => todo!(),
            RepoHostKind::Forgejo => todo!(),
        }
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
        Self::verify_webhook_signature(&parts, &body[..], &repository_host)?;

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
        let repo_id = Repository {
            host: repo_host_id,
            owner,
            name: repo,
        }
        .find(&self.db)
        .await?;
        let run_id = Run {
            created_at: start,
            commit: head_sha.clone(),
            repository: repo_id,
            dequeued_at: None,
            finished: None,
            target: String::from("ci"),
            rain_version: None,
        }
        .create(&self.db)
        .await?;

        self.tx.send(RunRequest { run_id }).await?;
        Ok(())
    }

    pub async fn handle_run_request(
        self: Arc<Self>,
        run_request: RunRequest,
    ) -> Result<(), anyhow::Error> {
        let run_id = run_request.run_id;
        let start = chrono::Utc::now();
        let run = Run::get(&self.db, run_id).await?;
        let repository = Repository::get(&self.db, run.resource.repository)
            .await?
            .resource;
        let repository_host = RepositoryHost::get(&self.db, repository.host)
            .await?
            .resource;

        let owner = repository.owner;
        let repo = repository.name;
        let head_sha = run.resource.commit.clone();

        match repository_host.kind {
            RepoHostKind::Github => {
                let github_client = AppClient::new(AppAuth {
                    app_id: AppId(
                        repository_host
                            .app_id
                            .context("no app id")?
                            .parse()
                            .context("invalid app id")?,
                    ),
                    key: jsonwebtoken::EncodingKey::from_rsa_pem(
                        repository_host
                            .app_key
                            .context("no app key")?
                            .expose_secret()
                            .as_bytes(),
                    )
                    .context("decode github app key")?,
                });

                self.handle_github_run_request(start, run, owner, repo, head_sha, github_client)
                    .await
            }
            RepoHostKind::Gitlab => todo!(),
            RepoHostKind::Forgejo => todo!(),
        }
    }

    #[expect(clippy::unwrap_used)]
    async fn handle_github_run_request(
        self: Arc<Self>,
        start: chrono::DateTime<Utc>,
        run: WithId<Run>,
        owner: String,
        repo: String,
        head_sha: String,
        github_client: AppClient,
    ) -> Result<()> {
        let installations = github_client.app_installations().await?;
        // FIXME: Getting the first installation is a bad assumption
        let installation = installations.first().unwrap();
        let installation_client = Arc::new(github_client.auth_installation(installation.id).await?);

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

        Run::dequeued(&self.db, run.id, env!("CARGO_PKG_VERSION"))
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
            .download_and_run(
                &installation_client,
                &owner,
                &repo,
                head_sha,
                run.resource.target,
            )
            .await;

        let (status, conclusion, output) = resolve_error(result_handle).await;

        let finished_at = Utc::now();
        let execution_time = finished_at - start;
        Run::finished(
            &self.db,
            run.id,
            FinishedRun {
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
            let config = rain_core::config::Config::new();
            let driver = rain_core::driver::DriverImpl::new(config);
            let download_area = driver
                .create_area(&[], &CreateAreaOptions::default())
                .unwrap();
            let download_entry =
                GeneratedFSEntry::new(download_area, SealedFilePath::new("/download").unwrap());
            std::fs::write(driver.resolve_fs_entry((&download_entry).into()), download).unwrap();
            let download = GeneratedFile::new_checked(&driver, download_entry).unwrap();
            let raw_tar = driver
                .extract_gzip(&File::Generated(download), "extract_temp.tar")
                .unwrap();
            let area = driver.extract_tar(&File::Generated(raw_tar)).unwrap();
            let mut ls = std::fs::read_dir(
                driver.resolve_fs_entry(GeneratedDir::root(area.clone()).fsinner().into()),
            )
            .unwrap();
            let entry = ls.next().unwrap().unwrap();
            let download_dir_name = entry.file_name().into_string().unwrap();
            let download_dir_entry =
                GeneratedFSEntry::new(area, SealedFilePath::new(&download_dir_name).unwrap());
            let root = GeneratedDir::new_checked(&driver, download_dir_entry).unwrap();
            let lfs_entries: Vec<_> = driver
                .glob(&Dir::Generated(root.clone()), "**/*")
                .unwrap()
                .into_iter()
                .filter_map(|entry| {
                    let path = driver.resolve_fs_entry(entry.fsinner());
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
                .create_overlay_area(
                    std::iter::once(root.fsinner().into()),
                    &CreateAreaOptions {
                        include_hidden: true,
                        flatten_input_dirs: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            let run_complete = server.runner.run(&driver, FSArea::Generated(area), &target);
            Ok(run_complete)
        }))
    }

    fn verify_webhook_signature(
        request: &Parts,
        body: &[u8],
        repo_host: &RepositoryHost,
    ) -> Result<()> {
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
            repo_host
                .webhook_secret
                .as_ref()
                .context("no webhook secret")?
                .expose_secret()
                .as_bytes(),
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
