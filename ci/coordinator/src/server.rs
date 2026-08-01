use std::{convert::Infallible, sync::Arc};

use alias::Alias as _;
use anyhow::{Context as _, Result};
use http::{Request, Response};
use hyper::body::Incoming;
use rain_ci_common::db::repository::Repository;
use rain_ci_common::db::repository_host::{RepoHostKind, RepositoryHost, RepositoryHostId};
use rain_ci_common::db::run::{FinishedRun, Run, RunId};
use rain_ci_common::db::{Db, Resource as _};
use rain_lang::cancellation::Cancellation;
use secrecy::ExposeSecret as _;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Receiver;
use tracing::{error, info, warn};
use url::Url;

use crate::repo_host::RepoHostApi as _;
use crate::runner::Runner;
use crate::{CancelRun, RunRequest};

pub struct Server {
    pub target_url: url::Url,
    pub runner: Runner,
    pub db: Db,
    pub run_request: tokio::sync::mpsc::Sender<RunRequest>,
    pub active_run: Mutex<Option<(RunId, Cancellation)>>,
}

impl Server {
    pub fn target_url(&self, id: RunId) -> Result<Url> {
        Ok(self.target_url.join(&format!("run/{id}"))?)
    }

    pub fn start_server_run_request_worker(self: &Arc<Self>, mut rx: Receiver<RunRequest>) {
        let server = self.alias();
        tokio::spawn(async move {
            loop {
                let Some(check_suite_event) = rx.recv().await else {
                    error!("server run request recv channel closed");
                    return;
                };
                if let Err(err) = server.alias().handle_run_request(check_suite_event).await {
                    error!("handle check suite event: {err}");
                }
            }
        });
    }

    pub fn start_server_cancel_run_worker(self: &Arc<Self>, mut rx: Receiver<CancelRun>) {
        let server = self.alias();
        tokio::spawn(async move {
            loop {
                let Some(cancel_run) = rx.recv().await else {
                    error!("server cancel run recv channel closed");
                    return;
                };
                let active_run = server.active_run.lock().await;
                let Some((run_id, cancel)) = &*active_run else {
                    warn!("no active run to cancel");
                    continue;
                };
                if run_id == &cancel_run.run_id {
                    cancel.cancel();
                    info!("cancelled active run");
                } else {
                    warn!("active run was not cancelled because it did not match the id");
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
        &self,
        repo_host_id: RepositoryHostId,
        request: Request<Incoming>,
    ) -> Result<()> {
        let repository_host = RepositoryHost::get(&self.db, repo_host_id).await?;

        match repository_host.resource.kind {
            RepoHostKind::Github => {
                let api = crate::github::Github::new(repository_host)?;
                api.handle_webhook(self, request).await
            }
            RepoHostKind::Forgejo => {
                let api = crate::forgejo::Forgejo::new(repository_host)?;
                api.handle_webhook(self, request).await
            }
        }
    }

    pub async fn handle_run_request(
        self: Arc<Self>,
        run_request: RunRequest,
    ) -> Result<(), anyhow::Error> {
        let run_id = run_request.run_id;
        let start = chrono::Utc::now();
        let run = Run::get(&self.db, run_id).await?;
        let repository = Repository::get(&self.db, run.resource.repository).await?;
        let repository_host = RepositoryHost::get(&self.db, repository.resource.host).await?;

        match repository_host.resource.kind {
            RepoHostKind::Github => {
                let api = crate::github::Github::new(repository_host)?;
                api.handle_run_request(self, run, repository, start).await
            }
            RepoHostKind::Forgejo => {
                let api = crate::forgejo::Forgejo::new(repository_host)?;
                api.handle_run_request(self, run, repository, start).await
            }
        }
    }

    pub async fn cleanup_old_runs(self: &Arc<Self>) -> Result<()> {
        let ids = sqlx::query!("SELECT id FROM runs LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run WHERE dequeued_at IS NOT NULL AND run IS NULL")
        .fetch_all(&self.db.pool)
        .await?;
        for row in ids {
            let run = Run::get(&self.db, RunId(row.id)).await?;
            let repository = Repository::get(&self.db, run.resource.repository).await?;
            let repository_host = RepositoryHost::get(&self.db, repository.resource.host).await?;
            let output = String::from("run was cleaned up on coordinator startup");
            let finished_at = chrono::Utc::now();
            let execution_time = chrono::TimeDelta::zero();
            match repository_host.resource.kind {
                RepoHostKind::Github => {
                    let api = crate::github::Github::new(repository_host)?;
                    api.finish_run(
                        self,
                        run,
                        repository,
                        rain_ci_common::db::run::RunStatus::SystemFailure,
                        output.clone(),
                        finished_at,
                        execution_time,
                    )
                    .await?;
                }
                RepoHostKind::Forgejo => {
                    let api = crate::forgejo::Forgejo::new(repository_host)?;
                    api.finish_run(
                        self,
                        run,
                        repository,
                        rain_ci_common::db::run::RunStatus::SystemFailure,
                        output.clone(),
                        finished_at,
                        execution_time,
                    )
                    .await?;
                }
            }

            Run::finished(
                &self.db,
                RunId(row.id),
                FinishedRun {
                    finished_at,
                    status: rain_ci_common::db::run::RunStatus::SystemFailure,
                    execution_time,
                    output,
                },
            )
            .await?;
        }
        Ok(())
    }
}

pub fn verify_webhook_signature(
    signature_hex: &str,
    body: &[u8],
    repo_host: &RepositoryHost,
) -> Result<()> {
    let sig = hex::decode(signature_hex).context("decode signature hex")?;
    let key = ring::hmac::Key::new(
        ring::hmac::HMAC_SHA256,
        repo_host
            .webhook_secret
            .as_ref()
            .context("no webhook secret configured")?
            .expose_secret()
            .as_bytes(),
    );
    ring::hmac::verify(&key, body, &sig).context("verify signature")?;
    Ok(())
}
