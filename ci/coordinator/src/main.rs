mod octocrab_extensions;

use std::{sync::Arc, time::Duration};

use anyhow::{Context as _, Result, anyhow};
use axum::http::HeaderMap;
use octocrab::{
    OctocrabBuilder,
    models::{
        StatusState, hooks,
        webhook_events::{WebhookEvent, WebhookEventPayload, WebhookEventType},
    },
};
use octocrab_extensions::{OctocrabExt as _, TreeEntry};
use smee_rs::{MessageHandler, default_smee_server_url};

#[derive(Debug, serde::Deserialize)]
struct Config {
    github_token: String,
    repository_owner: String,
    repository: String,
    target_url: url::Url,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let config = envy::from_env::<Config>()?;

    let crab = OctocrabBuilder::new()
        .personal_token(config.github_token)
        .build()?;

    let handler = Handler {
        inner: Arc::new(HandlerInner {
            crab: crab.clone(),
            owner: config.repository_owner.clone(),
            repo: config.repository.clone(),
            target_url: config.target_url.clone(),
        }),
    };
    let mut smee = smee_rs::Channel::new(default_smee_server_url(), handler).await?;
    let channel_url = smee.get_channel_url().to_string();
    tracing::info!("got channel url = {channel_url}");

    let hooks = crab
        .list_hooks(&config.repository_owner, &config.repository)
        .await?;
    tracing::info!("found {} hooks", hooks.items.len());

    for h in &hooks {
        crab.delete_hook(&config.repository_owner, &config.repository, h.id)
            .await?;
        tracing::info!("deleted hook {}", h.id);
    }

    crab.repos(&config.repository_owner, &config.repository)
        .create_hook(hooks::Hook {
            name: "web".to_owned(),
            config: hooks::Config {
                url: channel_url,
                content_type: Some(hooks::ContentType::Json),
                insecure_ssl: None,
                secret: None,
            },
            active: true,
            events: vec![WebhookEventType::Push],
            ..Default::default()
        })
        .await?;
    tracing::info!("created new hook");

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
}

impl MessageHandler for Handler {
    async fn handle(&self, headers: HeaderMap, body: String) -> Result<()> {
        let github_event_header = headers
            .get("x-github-event")
            .ok_or_else(|| anyhow!("x-github-event header not present"))?
            .to_str()?;
        let event = WebhookEvent::try_from_header_and_body(github_event_header, &body)?;
        let handler = Arc::clone(&self.inner);
        tokio::spawn(async move {
            if let Err(err) = Box::pin(handler.handle_hook(event)).await {
                tracing::error!("handle_hook error: {err}");
            }
        });

        Ok(())
    }
}

impl HandlerInner {
    async fn handle_hook(self: Arc<Self>, event: WebhookEvent) -> Result<()> {
        let event_repository = event
            .repository
            .ok_or_else(|| anyhow!("repository not present"))?;
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
        match event.specific {
            WebhookEventPayload::Push(push_event) => {
                tracing::info!("webhook push event");
                let head_commit = push_event
                    .head_commit
                    .as_ref()
                    .ok_or_else(|| anyhow!("no head commit"))?;
                self.crab
                    .repos(&self.owner, &self.repo)
                    .create_status(head_commit.id.clone(), StatusState::Pending)
                    .context("rain".to_owned())
                    .target(self.target_url.to_string())
                    .description("yippeeee".to_owned())
                    .send()
                    .await?;
                let tree = self
                    .crab
                    .get_tree(&self.owner, &self.repo, &head_commit.id)
                    .await?;
                let Some(readme) = tree.tree.iter().find_map(|t| match t {
                    TreeEntry::Blob { blob } if blob.path.eq_ignore_ascii_case("readme.md") => {
                        Some(blob)
                    }
                    _ => None,
                }) else {
                    self.crab
                        .repos(&self.owner, &self.repo)
                        .create_status(head_commit.id.clone(), StatusState::Failure)
                        .context("rain".to_owned())
                        .target(self.target_url.to_string())
                        .description("yippeeee".to_owned())
                        .send()
                        .await?;
                    return Ok(());
                };
                tracing::info!("tree {tree:#?}");
                let readme = self
                    .crab
                    .get_blob(&self.owner, &self.repo, &readme.sha)
                    .await?;
                assert_eq!(readme.encoding, "base64");
                tracing::info!("{readme:#?}");

                let readme = String::from_utf8(
                    base64::engine::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        readme.content.replace('\n', ""),
                    )
                    .context("base64 decode")?,
                )
                .context("utf8")?;
                tracing::info!("{readme}");
                tokio::time::sleep(Duration::from_secs(20)).await;
                self.crab
                    .repos(&self.owner, &self.repo)
                    .create_status(head_commit.id.clone(), StatusState::Success)
                    .context("rain".to_owned())
                    .target(self.target_url.to_string())
                    .description("yippeeee".to_owned())
                    .send()
                    .await?;
            }
            WebhookEventPayload::Ping(_) => {
                tracing::info!("webhook ping event");
            }
            _ => {
                tracing::warn!("unknown webhook event {kind:?}", kind = event.kind);
            }
        }
        Ok(())
    }
}
