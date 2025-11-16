use std::{
    borrow::Cow,
    io::{Read as _, Write as _},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use crate::{github::InstallationClient as _, runner::Runner};
use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use httparse::Request;
use log::{error, info, trace};
use rain_lang::afs::{dir::Dir, file::File};
use rain_lang::afs::{entry::FSEntry, entry::FSEntryTrait as _, path::SealedFilePath};
use rain_lang::driver::{DriverTrait as _, FSTrait as _};

use crate::runner::RunComplete;

const OK_REPSONSE: &str = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
const NOT_FOUND_RESPONSE: &str = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n";
const INTERNAL_ERR_RESPONSE: &str =
    "HTTP/1.1 500 INTERNAL SERVER ERROR\r\nContent-Length: 0\r\n\r\n";

pub struct Server<GH: crate::github::Client> {
    pub target_url: url::Url,
    pub github_webhook_secret: String,
    pub runner: Runner,
    pub github_client: GH,
    pub storage: Box<dyn crate::storage::StorageTrait>,
}

impl<GH: crate::github::Client> Server<GH> {
    pub fn handle_connection(&self, mut stream: TcpStream, addr: SocketAddr) -> Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
        trace!("connection {addr:?}");
        let mut buffer = [0u8; 1024];
        let len = stream.read(&mut buffer)?;
        let buffer = &buffer[..len];
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = Request::new(&mut headers);
        let parsed = request.parse(buffer)?;
        if parsed.is_partial() {
            return Err(anyhow!("partial http request"));
        }
        match request.version {
            Some(0 | 1) => {}
            v => return Err(anyhow!("invalid http version: {v:?}")),
        }
        match self.handle_request(&request, &mut stream, &buffer[parsed.unwrap()..]) {
            Ok(()) => {}
            Err(err) => {
                error!("handle request error: {err:#}");
                stream.write_all(INTERNAL_ERR_RESPONSE.as_bytes())?;
            }
        }
        Ok(())
    }

    fn handle_request(
        &self,
        request: &Request,
        stream: &mut TcpStream,
        body_prefix: &[u8],
    ) -> Result<()> {
        match request.path {
            Some("/webhook/github") => {
                self.handle_github_event(request, stream, body_prefix)?;
                stream.write_all(OK_REPSONSE.as_bytes())?;
            }
            _ => {
                stream.write_all(NOT_FOUND_RESPONSE.as_bytes())?;
            }
        }
        Ok(())
    }

    fn handle_github_event(
        &self,
        request: &Request,
        stream: &mut TcpStream,
        body_prefix: &[u8],
    ) -> Result<()> {
        let content_type = find_header(request, "Content-Type").context("missing content type")?;
        if content_type != b"application/json" {
            return Err(anyhow!("unexpected content type"));
        }
        let user_agent = find_header(request, "User-Agent").context("missing user agent")?;
        if !user_agent.starts_with(b"GitHub-Hookshot/") {
            return Err(anyhow!("unexpected user agent"));
        }
        let raw_body = get_body(request, stream, body_prefix)?;
        self.verify_webhook_signature(request, &raw_body)?;

        let event_kind = std::str::from_utf8(
            find_header(request, "x-github-event").context("missing github event kind")?,
        )?;

        if event_kind != "check_suite" {
            return Err(anyhow!("unexpected event kind: {event_kind:?}"));
        }

        let check_suite_event: crate::github::model::CheckSuiteEvent =
            serde_json::from_slice(&raw_body)?;

        self.handle_check_suite_event(&check_suite_event)
    }

    #[expect(clippy::unwrap_used)]
    pub fn handle_check_suite_event(
        &self,
        check_suite_event: &crate::github::model::CheckSuiteEvent,
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

        let installation_client = self
            .github_client
            .auth_installation(check_suite_event.installation.id)?;
        info!("received {check_suite_event:?}");

        let owner = &check_suite_event.repository.owner.login;
        let repo = &check_suite_event.repository.name;
        let head_sha = &check_suite_event.check_suite.head_sha;

        let check_run = installation_client
            .create_check_run(
                owner,
                repo,
                crate::github::model::CreateCheckRun {
                    name: String::from("rainci"),
                    head_sha: head_sha.into(),
                    status: crate::github::model::Status::Queued,
                    details_url: Some(self.target_url.to_string()),
                    output: None,
                },
            )
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
            .context("storage create run")?;

        info!("created check run {run_id}");

        self.storage
            .dequeued_run(&run_id)
            .context("storage dequeue run")?;

        installation_client
            .update_check_run(
                owner,
                repo,
                check_run.id,
                crate::github::model::PatchCheckRun {
                    status: Some(crate::github::model::Status::InProgress),
                    ..Default::default()
                },
            )
            .context("update check run")?;

        let result = std::thread::scope(|s| {
            s.spawn::<_, Result<RunComplete>>(|| {
                let download = installation_client
                    .download_repo_tar(owner, repo, head_sha)
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
                    .smudge_git_lfs(owner, repo, lfs_entries)
                    .context("smudge git lfs")?;
                let area = driver.create_area(&[root.inner()]).unwrap();
                let run_complete = self.runner.run(&driver, area);
                Ok(run_complete)
            })
            .join()
        });
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
            .context("storage finished run")?;
        installation_client
            .update_check_run(
                owner,
                repo,
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
            .context("update check run")?;

        self.runner.prune();

        Ok(())
    }

    fn verify_webhook_signature(&self, request: &Request, body: &[u8]) -> Result<()> {
        let raw_signature =
            find_header(request, "x-hub-signature-256").context("signature not present")?;
        let signature = std::str::from_utf8(raw_signature)?;
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

fn find_header<'buf>(request: &Request<'_, 'buf>, header_name: &str) -> Option<&'buf [u8]> {
    request
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(header_name))
        .map(|h| h.value)
}

fn get_body<'buf>(
    request: &Request,
    stream: &'buf mut TcpStream,
    body_prefix: &'buf [u8],
) -> Result<Cow<'buf, [u8]>> {
    const MAXIMUM_BODY_LENGTH: usize = 102_400; // 100 KiB
    let content_length: usize = std::str::from_utf8(
        find_header(request, "Content-Length").context("missing content length")?,
    )?
    .parse()?;
    if content_length > MAXIMUM_BODY_LENGTH {
        return Err(anyhow!("content length exceeds max length"));
    }
    if content_length <= body_prefix.len() {
        return Ok(Cow::Borrowed(&body_prefix[..content_length]));
    }
    let mut body = vec![0u8; content_length];
    stream.read_exact(&mut body[body_prefix.len()..])?;
    body[..body_prefix.len()].copy_from_slice(body_prefix);

    Ok(Cow::Owned(body))
}
