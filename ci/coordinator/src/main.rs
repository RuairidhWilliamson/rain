mod runner;

use std::{path::PathBuf, sync::Arc, time::Instant};

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http_body_util::BodyExt as _;
use jsonwebtoken::EncodingKey;
use octocrab::{
    OctocrabBuilder,
    models::{
        AppId, Author, Repository,
        orgs::Organization,
        webhook_events::{
            EventInstallation, WebhookEventPayload, WebhookEventType,
            payload::CheckSuiteWebhookEventAction,
        },
    },
    params::checks::{CheckRunOutput, CheckRunStatus},
};
use webhook_forwarder::{HeaderMap, MessageHandler};

#[derive(Debug, serde::Deserialize)]
struct Config {
    github_app_id: AppId,
    github_app_key: PathBuf,
    github_webhook_secret: String,
    target_url: url::Url,
    whf_server: Option<url::Url>,
    whf_channel_url: Option<url::Url>,
}

#[expect(clippy::unwrap_used)]
#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();
    tracing_subscriber::fmt::init();
    if let Err(err) = dotenv_result {
        tracing::warn!(".env could not be loaded: {err:#}");
    }
    let config = envy::from_env::<Config>()?;

    let key_raw = tokio::fs::read(&config.github_app_key).await.unwrap();
    let key = EncodingKey::from_rsa_pem(&key_raw).unwrap();

    octocrab::auth::AppAuth {
        app_id: config.github_app_id,
        key: key.clone(),
    }
    .generate_bearer_token()
    .unwrap();

    let crab = OctocrabBuilder::new()
        .app(config.github_app_id, key)
        .build()?;
    let installs = crab.apps().installations().send().await.unwrap();
    let perms = &installs.items.first().unwrap().permissions;
    tracing::info!("{perms:?}");

    let runner = runner::Runner::new();
    let handler = Handler {
        inner: Arc::new(HandlerInner {
            crab: crab.clone(),
            config,
            runner,
        }),
    };
    let mut channel;
    if let Some(channel_url) = handler.inner.config.whf_channel_url.as_ref() {
        channel = webhook_forwarder::Channel::from_existing_channel(channel_url.clone(), handler);
    } else {
        channel = webhook_forwarder::Channel::new(
            handler
                .inner
                .config
                .whf_server
                .clone()
                .context("whf server not set")?,
            handler,
        )
        .await
        .unwrap();
    }
    let channel_url = channel.get_channel_url().to_string();
    tracing::info!("got channel url = {channel_url}");

    tokio::select! {
        res = channel.start() => {res?}
        () = wait_signal()? => {}
    }
    Ok(())
}

// Intermediate structure allows to separate the common fields from
// the event specific one.
#[derive(serde::Deserialize)]
struct Intermediate {
    sender: Option<Author>,
    repository: Option<Repository>,
    organization: Option<Organization>,
    installation: Option<EventInstallation>,
    #[serde(flatten)]
    specific: serde_json::Value,
}

struct Handler {
    inner: Arc<HandlerInner>,
}

struct HandlerInner {
    crab: octocrab::Octocrab,
    config: Config,
    runner: runner::Runner,
}

impl MessageHandler for Handler {
    async fn handle(&self, headers: HeaderMap, body: Vec<u8>) -> Result<()> {
        verify_webhook_signature(&headers, &body, &self.inner.config.github_webhook_secret)?;
        let github_event_header = str::from_utf8(
            headers
                .get("x-github-event")
                .context("x-github-event header not present")?,
        )
        .context("x-github-event header is not a string")?;
        let header = github_event_header;
        // NOTE: this is inefficient code to simply reuse the code from "derived" serde::Deserialize instead
        // of writing specific deserialization code for the enum.
        let kind = if header.starts_with('"') {
            serde_json::from_str::<WebhookEventType>(header)?
        } else {
            serde_json::from_str::<WebhookEventType>(&format!("\"{header}\""))?
        };

        let Intermediate {
            sender,
            repository,
            organization,
            installation,
            specific,
        } = serde_json::from_slice(&body)?;

        let specific = kind.parse_specific_payload(specific)?;

        let event = WebhookEvent {
            sender,
            repository,
            organization,
            installation,
            kind,
            specific,
        };
        let handler = Arc::clone(&self.inner);
        tokio::spawn(async move {
            if let Err(err) = Box::pin(handler.handle_hook(event)).await {
                tracing::error!("handle_hook error: {err:#}");
            }
        });

        Ok(())
    }
}
/// A GitHub webhook event.
///
/// The structure is separated in common fields and specific fields, so you can
/// always access the common values without needing to match the exact variant.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct WebhookEvent {
    pub sender: Option<Author>,
    pub repository: Option<Repository>,
    pub organization: Option<Organization>,
    pub installation: Option<EventInstallation>,
    #[serde(skip)]
    pub kind: WebhookEventType,
    #[serde(flatten)]
    pub specific: WebhookEventPayload,
}

fn verify_webhook_signature(headers: &HeaderMap, body: &[u8], secret: &str) -> Result<()> {
    let github_signature_header = str::from_utf8(
        headers
            .get("x-hub-signature-256")
            .context("x-hub-signature-256 header not present")?,
    )
    .context("x-hub-signature-256 header is not a string")?;
    let (algo, sig_hex) = github_signature_header
        .split_once('=')
        .context("header does not contain =")?;
    if algo != "sha256" {
        return Err(anyhow!("unknown algorithm"));
    }
    let sig = hex::decode(sig_hex).context("decode signature hex")?;
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
    ring::hmac::verify(&key, body, &sig).context("verify signature")?;
    Ok(())
}

impl HandlerInner {
    async fn handle_hook(self: Arc<Self>, event: WebhookEvent) -> Result<()> {
        let installation_id = match event.installation {
            Some(EventInstallation::Full(installation)) => Some(installation.id),
            Some(EventInstallation::Minimal(event_installation_id)) => {
                Some(event_installation_id.id)
            }
            None => None,
        };
        match event.specific {
            WebhookEventPayload::Push(_push_event) => {
                tracing::info!("webhook push event");
            }
            WebhookEventPayload::Ping(_) => {
                tracing::info!("webhook ping event");
            }
            WebhookEventPayload::CheckSuite(suite_event) => {
                tracing::info!("check suite event {:?}", suite_event.action);
                match suite_event.action {
                    CheckSuiteWebhookEventAction::Rerequested
                    | CheckSuiteWebhookEventAction::Requested => (),
                    _ => return Ok(()),
                }
                let installation = self
                    .crab
                    .installation(installation_id.context("installation_id not present")?)?;
                let repository = event.repository.context("no repository")?;
                let owner = &repository.owner.context("no owner")?.login;
                let repo = &repository.name;
                let head_sha = suite_event
                    .check_suite
                    .get("head_sha")
                    .context("head_sha not present")?
                    .as_str()
                    .context("head_sha not string")?;
                Self::handle_check_suite_request(
                    &self.runner,
                    installation,
                    &self.config.target_url,
                    owner,
                    repo,
                    head_sha,
                )
                .await?;
            }
            WebhookEventPayload::CheckRun(_run_event) => {
                tracing::info!("check run event");
            }
            _ => {
                tracing::warn!("unknown webhook event {kind:?}", kind = event.kind);
            }
        }
        Ok(())
    }

    async fn handle_check_suite_request(
        runner: &runner::Runner,
        installation_crab: octocrab::Octocrab,
        target_url: &url::Url,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<()> {
        let check_run_name = "rain-test";
        let checks = installation_crab.checks(owner, repo);
        let check_run = checks
            .create_check_run(check_run_name, head_sha)
            .status(CheckRunStatus::Queued)
            .details_url(target_url.to_string())
            .output(CheckRunOutput {
                title: String::from("Rain CI Run"),
                summary: String::from("Summary..."),
                text: None,
                annotations: vec![],
                images: vec![],
            })
            .send()
            .await?;
        let check_run = checks
            .update_check_run(check_run.id)
            .status(CheckRunStatus::InProgress)
            .details_url(target_url.to_string())
            .output(CheckRunOutput {
                title: String::from("Rain CI Run"),
                summary: String::from("Summary..."),
                text: None,
                annotations: vec![],
                images: vec![],
            })
            .send()
            .await?;
        let start = Instant::now();
        let download = installation_crab
            .repos(owner, repo)
            .download_tarball(head_sha.to_owned())
            .await?;
        let download = download.collect().await?.to_bytes();
        let runner::RunComplete { success, output } =
            Self::run(runner, &download, owner, repo, head_sha).await;
        let conclusion = if success {
            octocrab::params::checks::CheckRunConclusion::Success
        } else {
            octocrab::params::checks::CheckRunConclusion::Failure
        };
        let elapsed = start.elapsed();
        checks
            .update_check_run(check_run.id)
            .status(CheckRunStatus::Completed)
            .conclusion(conclusion)
            .completed_at(Utc::now())
            .details_url(target_url.to_string())
            .output(CheckRunOutput {
                title: String::from("Rain CI Run"),
                summary: format!("Completed in {elapsed:.01?}"),
                text: Some(format!("```\n{output}\n```")),
                annotations: vec![],
                images: vec![],
            })
            .send()
            .await?;
        Ok(())
    }

    async fn run(
        runner: &runner::Runner,
        download: &[u8],
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> runner::RunComplete {
        // Need to do this to satisfy static lifetime
        let download = download.to_vec();
        let download_dir_name = format!("{owner}-{repo}-{head_sha}");
        let runner = runner.clone();
        tokio::task::spawn_blocking(move || runner.run(&download, &download_dir_name))
            .await
            .unwrap_or_else(|err| {
                tracing::error!("panic: {err}");
                runner::RunComplete {
                    success: false,
                    output: "something panicked!".into(),
                }
            })
    }
}

#[cfg(target_family = "windows")]
fn wait_signal() -> Result<impl Future<Output = ()>> {
    use tokio::signal::windows;
    let mut ctrl_c = windows::ctrl_c()?;
    Ok(async move {
        ctrl_c.recv().await;
        tracing::warn!("caught CTRL+C, exiting...");
    })
}

#[cfg(target_family = "unix")]
fn wait_signal() -> Result<impl Future<Output = ()>> {
    use tokio::signal::unix;
    let mut sigterm = unix::signal(unix::SignalKind::terminate())?;
    let mut sigint = unix::signal(unix::SignalKind::interrupt())?;

    Ok(async move {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::warn!("caught SIGTERM, exiting...");
            },
            _ = sigint.recv() => {
                tracing::warn!("caught SIGINT, exiting...");
            },
        }
    })
}
