use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http::Request;
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use log::info;
use rain_ci_common::{
    db::{
        WithId,
        repository::Repository,
        repository_host::{RepositoryHost, RepositoryHostId},
        run::{FinishedRun, Run, RunStatus},
    },
    github::{
        Client as _, InstallationClient as _, implementation::AppClient, model::CheckRunConclusion,
    },
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
use tokio::task::JoinHandle;

use crate::{RunRequest, runner::RunComplete, server::Server};

pub async fn handle_webhook(
    server: &Server,
    repo_host_id: RepositoryHostId,
    request: Request<Incoming>,
    repository_host: &RepositoryHost,
) -> Result<()> {
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
    let signature = parts
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
    crate::server::verify_webhook_signature(sig_hex, &body[..], repository_host)?;

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
    let sha = check_suite_event.check_suite.head_sha;

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
    github_client: AppClient,
) -> Result<()> {
    let target_url = server.target_url(run.id)?;
    let installations = github_client.app_installations().await?;
    // FIXME: Getting the first installation is a bad assumption
    let installation = installations.first().context("no installations")?;
    let installation_client = Arc::new(github_client.auth_installation(installation.id).await?);

    let check_run = installation_client
        .create_check_run(
            &owner,
            &repo,
            rain_ci_common::github::model::CreateCheckRun {
                name: String::from("rainci"),
                head_sha: sha.clone(),
                status: rain_ci_common::github::model::Status::Queued,
                details_url: Some(target_url.to_string()),
                output: None,
            },
        )
        .await
        .context("create check run")?;

    Run::dequeued(&server.db, run.id, env!("CARGO_PKG_VERSION"))
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
    let result_handle = download_and_run(
        &server,
        &installation_client,
        &owner,
        &repo,
        sha,
        run.resource.target,
    )
    .await;

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

    server.runner.prune();

    Ok(())
}

async fn download_and_run(
    server: &Arc<Server>,
    installation_client: &Arc<impl rain_ci_common::github::InstallationClient>,
    owner: &str,
    repo: &str,
    sha: String,
    target: String,
) -> Result<JoinHandle<Result<RunComplete, anyhow::Error>>, anyhow::Error> {
    let server = Arc::clone(server);
    let installation_client = Arc::clone(installation_client);
    let download = installation_client
        .download_repo_tar(owner, repo, &sha)
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
