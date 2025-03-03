mod octocrab_extensions;

use std::{sync::Arc, time::Duration};

use anyhow::{Result, anyhow};
use axum::http::HeaderMap;
use octocrab::{
    OctocrabBuilder,
    models::{
        StatusState, hooks,
        webhook_events::{WebhookEvent, WebhookEventPayload, WebhookEventType},
    },
};
use octocrab_extensions::OctocrabExt as _;
use smee_rs::{MessageHandler, default_smee_server_url};

fn get_env_var(key: &str) -> String {
    std::env::var(key).expect(key)
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let gh_token = get_env_var("GH_TOKEN");
    let owner = get_env_var("REPO_OWNER");
    let repo = get_env_var("REPO");

    let crab = OctocrabBuilder::new().personal_token(gh_token).build()?;

    let handler = Handler {
        inner: Arc::new(HandlerInner {
            crab: crab.clone(),
            owner: owner.clone(),
            repo: repo.clone(),
        }),
    };
    let mut smee = smee_rs::Channel::new(default_smee_server_url(), handler).await?;
    let channel_url = smee.get_channel_url().to_string();
    tracing::info!("got channel url = {channel_url}");

    let hooks = crab.list_hooks(&owner, &repo).await?;
    tracing::info!("found {} hooks", hooks.items.len());

    for h in &hooks {
        crab.delete_hook(&owner, &repo, h.id).await?;
        tracing::info!("deleted hook {}", h.id);
    }

    crab.repos(&owner, &repo)
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
        let handler = self;
        let event_repository = event
            .repository
            .ok_or_else(|| anyhow!("repository not present"))?;
        if event_repository.full_name
            != Some(format!(
                "{owner}/{repo}",
                owner = &handler.owner,
                repo = &handler.repo
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
                handler
                    .crab
                    .repos(&handler.owner, &handler.repo)
                    .create_status(head_commit.id.clone(), StatusState::Pending)
                    .context("rain".to_owned())
                    .target("https://example.com".to_owned())
                    .description("yippeeee".to_owned())
                    .send()
                    .await?;
                tokio::time::sleep(Duration::from_secs(20)).await;
                handler
                    .crab
                    .repos(&handler.owner, &handler.repo)
                    .create_status(head_commit.id.clone(), StatusState::Success)
                    .context("rain".to_owned())
                    .target("https://example.com".to_owned())
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
