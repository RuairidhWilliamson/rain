use std::sync::Arc;

use alias::Alias as _;
use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http::Request;
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use log::info;
use rain_ci_common::db::{
    WithId,
    repository::Repository,
    repository_host::{RepositoryHost, RepositoryHostId},
    run::{FinishedRun, Run, RunStatus},
};
use rain_lang::{
    afs::{
        Dir, File,
        area::FSArea,
        generated::{dir::GeneratedDir, entry::GeneratedFSEntry, file::GeneratedFile},
        path::SealedFilePath,
    },
    driver::{CreateAreaOptions, DriverTrait as _, FSTrait as _},
};
use serde::Deserialize;
use tokio::task::JoinHandle;

use crate::{RunRequest, runner::RunComplete, server::Server};

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

pub async fn handle_webhook(
    server: &Server,
    repo_host_id: RepositoryHostId,
    request: Request<Incoming>,
    repository_host: RepositoryHost,
) -> Result<()> {
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
        &repository_host,
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
        host: repo_host_id,
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
    }
    .create(&server.db)
    .await?;

    server.tx.send(RunRequest { run_id }).await?;
    Ok(())
}

pub async fn handle_run_request(
    server: Arc<Server>,
    start: chrono::DateTime<Utc>,
    run: WithId<Run>,
    owner: String,
    repo: String,
    sha: String,
    forgejo: forgejo_api::Forgejo,
) -> Result<()> {
    let context = format!("rain-run-{}", run.id);
    let target_url = server.target_url(run.id)?;
    forgejo
        .repo_create_status(
            &owner,
            &repo,
            &sha,
            forgejo_api::structs::CreateStatusOption {
                context: Some(context.clone()),
                description: Some("rain run queued".into()),
                state: Some(forgejo_api::structs::CommitStatusState::Pending),
                target_url: Some(target_url.clone()),
            },
        )
        .await?;

    Run::dequeued(&server.db, run.id, env!("CARGO_PKG_VERSION"))
        .await
        .context("storage dequeue run")?;

    forgejo
        .repo_create_status(
            &owner,
            &repo,
            &sha,
            forgejo_api::structs::CreateStatusOption {
                context: Some(context.clone()),
                description: Some("rain run in progress".into()),
                state: Some(forgejo_api::structs::CommitStatusState::Pending),
                target_url: Some(target_url.clone()),
            },
        )
        .await?;

    log::info!("Preparing run");

    let result_handle =
        download_and_run(&server, &forgejo, &owner, &repo, &sha, run.resource.target).await;

    let (status, conclusion, output) = resolve_error(result_handle).await;

    let finished_at = Utc::now();
    let execution_time = finished_at - start;
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
    forgejo
        .repo_create_status(
            &owner,
            &repo,
            &sha,
            forgejo_api::structs::CreateStatusOption {
                context: Some(context.clone()),
                description: Some("rain run complete".into()),
                state: Some(conclusion),
                target_url: Some(target_url.clone()),
            },
        )
        .await?;
    server.runner.prune();
    Ok(())
}

async fn download_and_run(
    server: &Arc<Server>,
    forgejo: &forgejo_api::Forgejo,
    owner: &str,
    repo: &str,
    sha: &str,
    target: String,
) -> Result<JoinHandle<Result<RunComplete, anyhow::Error>>, anyhow::Error> {
    let server = (server).alias();
    let download = forgejo
        .repo_get_archive(owner, repo, &format!("{sha}.zip"))
        .await?;
    #[expect(clippy::unwrap_used)]
    let (root, _lfs_entries) = tokio::task::spawn_blocking(move || {
        let config = rain_core::config::Config::new();
        let driver = rain_core::driver::DriverImpl::new(config);
        let download_area = driver
            .create_area(&[], &CreateAreaOptions::default())
            .unwrap();
        let download_entry =
            GeneratedFSEntry::new(download_area, SealedFilePath::new("/download").unwrap());
        std::fs::write(driver.resolve_fs_entry((&download_entry).into()), download).unwrap();
        let download = GeneratedFile::new_checked(&driver, download_entry).unwrap();
        let area = driver.extract_zip(&File::Generated(download)).unwrap();
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
    // TODO: Smudge lfs
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

async fn resolve_error(
    result_handle: Result<JoinHandle<Result<RunComplete>>>,
) -> (RunStatus, forgejo_api::structs::CommitStatusState, String) {
    match result_handle {
        Ok(handle) => {
            let result: Result<Result<RunComplete>, _> = handle.await;
            match result {
                Ok(Ok(RunComplete {
                    success: true,
                    output,
                })) => (
                    RunStatus::Success,
                    forgejo_api::structs::CommitStatusState::Success,
                    output,
                ),
                Ok(Ok(RunComplete {
                    success: false,
                    output,
                })) => (
                    RunStatus::Failure,
                    forgejo_api::structs::CommitStatusState::Error,
                    output,
                ),
                Ok(Err(err)) => {
                    log::error!("runner error: {err:?}");
                    (
                        RunStatus::Failure,
                        forgejo_api::structs::CommitStatusState::Failure,
                        String::default(),
                    )
                }
                Err(err) => {
                    log::error!("runner panicked: {err:?}");
                    (
                        RunStatus::Failure,
                        forgejo_api::structs::CommitStatusState::Failure,
                        String::default(),
                    )
                }
            }
        }
        Err(err) => {
            log::error!("runner download error: {err:?}");
            (
                RunStatus::Failure,
                forgejo_api::structs::CommitStatusState::Failure,
                String::default(),
            )
        }
    }
}
