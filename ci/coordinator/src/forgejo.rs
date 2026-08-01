use std::sync::Arc;

use alias::Alias as _;
use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http::{
    Request,
    header::{ACCEPT, CONTENT_TYPE},
};
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use rain_ci_common::db::{
    WithId,
    repository::Repository,
    repository_host::RepositoryHost,
    run::{FinishedRun, Run, RunStatus},
};
use rain_lang::cancellation::Cancellation;
use secrecy::ExposeSecret as _;
use serde::Deserialize;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::{
    RunRequest,
    repo_host::RepoHostApi,
    runner::{RunComplete, RunOptions},
    server::Server,
};

pub struct Forgejo {
    repository_host: WithId<RepositoryHost>,
    api: forgejo_api::Forgejo,
    client: reqwest::Client,
    url: url::Url,
    username: String,
    token: String,
}

impl Forgejo {
    pub fn new(repository_host: WithId<RepositoryHost>) -> Result<Self> {
        let url = url::Url::parse(&repository_host.resource.url)?;
        let client = reqwest::ClientBuilder::new().build()?;
        let username = repository_host
            .resource
            .app_id
            .clone()
            .context("app id username not set")?;
        let token = repository_host
            .resource
            .app_key
            .as_ref()
            .context("app key not set")?
            .expose_secret()
            .to_owned();
        let api = forgejo_api::Forgejo::new(forgejo_api::Auth::Token(&token), url.clone())?;
        Ok(Self {
            repository_host,
            api,
            client,
            url,
            username,
            token,
        })
    }

    async fn smudge_git_lfs(
        &self,
        owner: &str,
        repo: &str,
        entries: Vec<(std::path::PathBuf, git_lfs_rs::object::Object)>,
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let request = git_lfs_rs::api::Request {
            operation: git_lfs_rs::api::Operation::Download,
            transfers: vec![git_lfs_rs::api::Transfer::Basic],
            r#ref: None,
            objects: entries.iter().map(|(_, o)| o.into()).collect(),
            hash_algo: git_lfs_rs::api::HashAlgorithm::Sha256,
        };
        let response = self
            .git_lfs_api(owner, repo, request)
            .await
            .context("git lfs api")?;
        for (resp, (path, _)) in response.objects.into_iter().zip(entries) {
            let mut f = tokio::fs::File::create(&path)
                .await
                .context("create lfs file")?;
            let body = self
                .client
                .get(
                    &resp
                        .actions
                        .get(&git_lfs_rs::api::Operation::Download)
                        .context("no download action")?
                        .href,
                )
                .basic_auth(self.username.clone(), Some(self.token.clone()))
                .send()
                .await
                .context("download lfs object")?
                .error_for_status()?
                .bytes()
                .await?;
            let mut reader = &body[..];
            tokio::io::copy(&mut reader, &mut f).await?;
        }
        Ok(())
    }

    async fn git_lfs_api(
        &self,
        owner: &str,
        repo: &str,
        request: git_lfs_rs::api::Request,
    ) -> Result<git_lfs_rs::api::Response> {
        let url = self
            .url
            .join(&format!("/{owner}/{repo}.git/info/lfs/objects/batch"))?;
        let response: git_lfs_rs::api::Response = self
            .client
            .post(url)
            .basic_auth(self.username.clone(), Some(self.token.clone()))
            .header(CONTENT_TYPE, "application/vnd.git-lfs+json")
            .header(ACCEPT, "application/vnd.git-lfs+json")
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response)
    }

    async fn download_and_run(
        &self,
        server: &Arc<Server>,
        owner: &str,
        repo: &str,
        options: RunOptions,
    ) -> Result<JoinHandle<Result<RunComplete, anyhow::Error>>, anyhow::Error> {
        let server = server.alias();
        let download = self
            .api
            .repo_get_archive(owner, repo, &format!("{}.zip", options.sha))
            .await?;
        let (root, lfs_entries) =
            tokio::task::spawn_blocking(move || crate::prepare::prepare_ci_run_area_zip(download))
                .await?;
        self.smudge_git_lfs(owner, repo, lfs_entries).await?;
        info!("Prepare run complete");
        Ok(tokio::task::spawn_blocking(move || {
            Ok(server.runner.run(&root, options))
        }))
    }
}

impl RepoHostApi for Forgejo {
    async fn handle_webhook(&self, server: &Server, request: Request<Incoming>) -> Result<()> {
        let headers = request.headers();
        let content_type = headers
            .get("Content-Type")
            .context("missing content type")?;
        if content_type.as_bytes() != b"application/json" {
            return Err(anyhow!("unexpected content type"));
        }
        let (parts, body) = request.into_parts();
        let body = body.collect().await?.to_bytes();
        crate::server::verify_webhook_signature(
            parts
                .headers
                .get("x-forgejo-signature")
                .context("signature not present")?
                .to_str()?,
            &body[..],
            &self.repository_host.resource,
        )?;

        let event_kind = parts
            .headers
            .get("x-forgejo-event")
            .context("missing forgejo event kind")?
            .to_str()?;

        if event_kind != "push" {
            return Err(anyhow!("unexpected event kind: {event_kind:?}"));
        }

        let push_event: crate::forgejo::ForgejoPushEvent = serde_json::from_slice(&body[..])?;

        info!("received {push_event:?}");

        let owner = push_event.repository.owner.login;
        let repo = push_event.repository.name;
        let sha = push_event.after;

        let start = chrono::Utc::now();
        let repo_id = Repository {
            host: self.repository_host.id,
            owner,
            name: repo,
        }
        .find(&server.db)
        .await?;
        let run_id = Run {
            created_at: start,
            commit: sha.clone(),
            repository: repo_id,
            dequeued_at: None,
            finished: None,
            target: String::from("ci"),
            rain_version: None,
            check_run_id: None,
        }
        .create(&server.db)
        .await?;

        server.run_request.send(RunRequest { run_id }).await?;
        Ok(())
    }

    async fn handle_run_request(
        &self,
        server: Arc<Server>,
        run: WithId<Run>,
        repository: WithId<Repository>,
        start: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let context = format!("rain-run-{}", run.id);
        let target_url = server.target_url(run.id)?;
        self.api
            .repo_create_status(
                &repository.resource.owner,
                &repository.resource.name,
                &run.resource.commit,
                forgejo_api::structs::CreateStatusOption {
                    context: Some(context.clone()),
                    description: Some("rain run queued".into()),
                    state: Some(forgejo_api::structs::CommitStatusState::Pending),
                    target_url: Some(target_url.to_string()),
                },
            )
            .await?;

        Run::dequeued(&server.db, run.id, env!("CARGO_PKG_VERSION"), None)
            .await
            .context("storage dequeue run")?;

        self.api
            .repo_create_status(
                &repository.resource.owner,
                &repository.resource.name,
                &run.resource.commit,
                forgejo_api::structs::CreateStatusOption {
                    context: Some(context.clone()),
                    description: Some("rain run in progress".into()),
                    state: Some(forgejo_api::structs::CommitStatusState::Pending),
                    target_url: Some(target_url.to_string()),
                },
            )
            .await?;

        info!("Preparing run");
        let secrets = Repository::get_secrets(&server.db, repository.id)
            .await
            .context("get repo secrets")?;

        let cancel = Cancellation::new();
        let mut active_run = server.active_run.lock().await;
        if active_run.is_some() {
            error!("overwriting active run");
        }
        *active_run = Some((run.id, cancel.clone()));
        drop(active_run);
        let result_handle = self
            .download_and_run(
                &server,
                &repository.resource.owner,
                &repository.resource.name,
                RunOptions {
                    sha: run.resource.commit.clone(),
                    target: run.resource.target.clone(),
                    secrets,
                    cancel,
                },
            )
            .await;
        let mut active_run = server.active_run.lock().await;
        if let Some((run_id, _)) = active_run.take() {
            if run_id != run.id {
                error!("active run id changed during run");
            }
        } else {
            error!("active run was removed during run");
        }
        drop(active_run);

        let (status, output) = resolve_error(result_handle).await;

        let finished_at = Utc::now();
        let execution_time = finished_at - start;
        self.finish_run(
            &server,
            run,
            repository,
            status,
            output,
            finished_at,
            execution_time,
        )
        .await?;
        server.runner.prune();
        Ok(())
    }

    async fn finish_run(
        &self,
        server: &Arc<Server>,
        run: WithId<Run>,
        repository: WithId<Repository>,
        status: RunStatus,
        output: String,
        finished_at: chrono::DateTime<Utc>,
        execution_time: chrono::TimeDelta,
    ) -> Result<()> {
        let context = format!("rain-run-{}", run.id);
        let target_url = server.target_url(run.id)?;
        let conclusion = match status {
            RunStatus::Success => forgejo_api::structs::CommitStatusState::Success,
            RunStatus::Failure => forgejo_api::structs::CommitStatusState::Failure,
            RunStatus::SystemFailure => forgejo_api::structs::CommitStatusState::Error,
        };
        Run::finished(
            &server.db,
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
        self.api
            .repo_create_status(
                &repository.resource.owner,
                &repository.resource.name,
                &run.resource.commit,
                forgejo_api::structs::CreateStatusOption {
                    context: Some(context.clone()),
                    description: Some("rain run complete".into()),
                    state: Some(conclusion),
                    target_url: Some(target_url.to_string()),
                },
            )
            .await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ForgejoPushEvent {
    pub after: String,
    pub repository: ForgejoRepository,
}

#[derive(Debug, Deserialize)]
pub struct ForgejoRepository {
    pub owner: ForgejoOwner,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ForgejoOwner {
    pub login: String,
}

async fn resolve_error(
    result_handle: Result<JoinHandle<Result<RunComplete>>>,
) -> (RunStatus, String) {
    match result_handle {
        Ok(handle) => {
            let result: Result<Result<RunComplete>, _> = handle.await;
            match result {
                Ok(Ok(RunComplete {
                    success: true,
                    output,
                })) => (RunStatus::Success, output),
                Ok(Ok(RunComplete {
                    success: false,
                    output,
                })) => (RunStatus::Failure, output),
                Ok(Err(err)) => {
                    error!("runner error: {err:?}");
                    (RunStatus::Failure, String::default())
                }
                Err(err) => {
                    error!("runner panicked: {err:?}");
                    (RunStatus::Failure, String::default())
                }
            }
        }
        Err(err) => {
            error!("runner download error: {err:?}");
            (RunStatus::Failure, String::default())
        }
    }
}
