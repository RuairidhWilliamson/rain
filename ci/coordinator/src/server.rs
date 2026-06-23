use std::{convert::Infallible, sync::Arc};

use alias::Alias as _;
use anyhow::{Context as _, Result};
use http::{Request, Response};
use hyper::body::Incoming;
use log::error;
use rain_ci_common::db::repository::Repository;
use rain_ci_common::db::repository_host::{RepoHostKind, RepositoryHost, RepositoryHostId};
use rain_ci_common::db::run::{Run, RunId};
use rain_ci_common::db::{Db, Resource as _};
use rain_ci_common::github::implementation::{AppAuth, AppClient};
use rain_ci_common::github::model::AppId;
use secrecy::ExposeSecret as _;
use tokio::sync::mpsc::Receiver;
use url::Url;

use crate::RunRequest;
use crate::runner::Runner;

pub struct Server {
    pub target_url: url::Url,
    pub runner: Runner,
    pub db: Db,
    pub tx: tokio::sync::mpsc::Sender<RunRequest>,
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
                    error!("server recv channel closed");
                    return;
                };
                if let Err(err) = server.alias().handle_run_request(check_suite_event).await {
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
        &self,
        repo_host_id: RepositoryHostId,
        request: Request<Incoming>,
    ) -> Result<()> {
        let repository_host = RepositoryHost::get(&self.db, repo_host_id).await?.resource;

        match repository_host.kind {
            RepoHostKind::Github => {
                crate::github::handle_webhook(self, repo_host_id, request, &repository_host).await
            }
            RepoHostKind::Forgejo => {
                crate::forgejo::handle_webhook(self, repo_host_id, request, repository_host).await
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
        let repository = Repository::get(&self.db, run.resource.repository)
            .await?
            .resource;
        let repository_host = RepositoryHost::get(&self.db, repository.host)
            .await?
            .resource;

        let owner = repository.owner;
        let repo = repository.name;
        let sha = run.resource.commit.clone();

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

                crate::github::handle_run_request(self, start, run, owner, repo, sha, github_client)
                    .await
            }
            RepoHostKind::Forgejo => {
                let forgejo = crate::forgejo::Forgejo::new(
                    &repository_host.url,
                    repository_host.app_key.context("no token")?.expose_secret(),
                )
                .context("create forgejo api client")?;
                crate::forgejo::handle_run_request(self, start, run, owner, repo, sha, forgejo)
                    .await
            }
        }
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
