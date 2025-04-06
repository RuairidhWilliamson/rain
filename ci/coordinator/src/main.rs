mod runner;

use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use http_body_util::BodyExt as _;
use jsonwebtoken::EncodingKey;
use octocrab::{
    OctocrabBuilder,
    models::{
        AppId,
        webhook_events::{
            EventInstallation, WebhookEvent, WebhookEventPayload,
            payload::CheckSuiteWebhookEventAction,
        },
    },
};
use smee_rs::MessageHandler;

#[derive(Debug, serde::Deserialize)]
struct Config {
    github_app_id: AppId,
    github_app_key: String,
    repository_owner: String,
    repository: String,
    target_url: url::Url,
    smee_url: url::Url,
}

#[expect(clippy::unwrap_used)]
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let config = envy::from_env::<Config>()?;

    let key_raw = tokio::fs::read(config.github_app_key).await.unwrap();
    let key = EncodingKey::from_rsa_pem(&key_raw).unwrap();

    octocrab::auth::AppAuth {
        app_id: config.github_app_id,
        key: key.clone(),
    }
    .generate_bearer_token()
    .unwrap();

    let crab = OctocrabBuilder::new()
        .app(config.github_app_id, key)
        // .personal_token(config.github_token)
        .build()?;
    let installs = crab.apps().installations().send().await.unwrap();
    let perms = &installs.items.first().unwrap().permissions;
    tracing::info!("{perms:?}");

    let runner = runner::Runner::new();
    let handler = Handler {
        inner: Arc::new(HandlerInner {
            crab: crab.clone(),
            owner: config.repository_owner.clone(),
            repo: config.repository.clone(),
            target_url: config.target_url.clone(),
            runner,
        }),
    };
    // let mut smee = smee_rs::Channel::new(default_smee_server_url(), handler).await?;
    let mut smee = smee_rs::Channel::from_existing_channel(config.smee_url, handler);
    let channel_url = smee.get_channel_url().to_string();
    tracing::info!("got channel url = {channel_url}");

    smee.start().await
}

struct Handler {
    inner: Arc<HandlerInner>,
}

struct HandlerInner {
    crab: octocrab::Octocrab,
    owner: String,
    repo: String,
    target_url: url::Url,
    runner: runner::Runner,
}

impl MessageHandler for Handler {
    async fn handle(&self, headers: &smee_rs::HeaderMap, body: String) -> Result<()> {
        let github_event_header = headers
            .get("x-github-event")
            .ok_or_else(|| anyhow!("x-github-event header not present"))?
            .as_str()
            .ok_or_else(|| anyhow!("x-github-event header is not a string"))?;
        let event = WebhookEvent::try_from_header_and_body(github_event_header, &body)?;
        let handler = Arc::clone(&self.inner);
        tokio::spawn(async move {
            if let Err(err) = Box::pin(handler.handle_hook(event)).await {
                tracing::error!("handle_hook error: {err:#}");
            }
        });

        Ok(())
    }
}

impl HandlerInner {
    async fn handle_hook(self: Arc<Self>, event: WebhookEvent) -> Result<()> {
        let event_repository = event.repository.ok_or_else(|| {
            tracing::info!("{:?}", event.specific);
            anyhow!("repository not present")
        })?;
        if event_repository.full_name
            != Some(format!(
                "{owner}/{repo}",
                owner = &self.owner,
                repo = &self.repo
            ))
        {
            return Err(anyhow!(
                "repository did not match expected, {:?}",
                event_repository.full_name
            ));
        }
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
                let head_sha = suite_event
                    .check_suite
                    .get("head_sha")
                    .context("head_sha not present")?
                    .as_str()
                    .context("head_sha not string")?;
                self.handle_check_suite_request(installation, head_sha)
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
        &self,
        installation_crab: octocrab::Octocrab,
        head_sha: &str,
    ) -> Result<()> {
        let check_run_name = "rain-test";
        let checks = installation_crab.checks(&self.owner, &self.repo);
        let check_run = checks
            .create_check_run(check_run_name, head_sha)
            .status(octocrab::params::checks::CheckRunStatus::InProgress)
            .details_url(self.target_url.to_string())
            .output(octocrab::params::checks::CheckRunOutput {
                title: String::from("Rain CI Run"),
                summary: String::from("Summary..."),
                text: None,
                annotations: vec![],
                images: vec![],
            })
            .send()
            .await?;
        let download = installation_crab
            .repos(&self.owner, &self.repo)
            .download_tarball(head_sha.to_owned())
            .await?;
        let download = download.collect().await?.to_bytes();
        let runner::RunComplete { success, output } = self.run(&download, head_sha).await?;
        let conclusion = if success {
            octocrab::params::checks::CheckRunConclusion::Success
        } else {
            octocrab::params::checks::CheckRunConclusion::Failure
        };
        checks
            .update_check_run(check_run.id)
            .status(octocrab::params::checks::CheckRunStatus::Completed)
            .conclusion(conclusion)
            .completed_at(Utc::now())
            .details_url(self.target_url.to_string())
            .output(octocrab::params::checks::CheckRunOutput {
                title: String::from("Rain CI Run"),
                summary: String::from("Summary..."),
                text: Some(output),
                annotations: vec![],
                images: vec![],
            })
            .send()
            .await?;
        Ok(())
    }

    async fn run(&self, download: &[u8], head_sha: &str) -> Result<runner::RunComplete> {
        // Need to do this to satisfy static lifetime
        let download = download.to_vec();
        let download_dir_name = format!("{}-{}-{}", self.owner, self.repo, head_sha);
        let runner = self.runner.clone();
        let complete =
            tokio::task::spawn_blocking(move || runner.run(&download, &download_dir_name)).await?;
        Ok(complete)
    }
}
