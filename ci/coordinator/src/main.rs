mod octocrab_extensions;

use std::sync::Arc;

use anyhow::{Result, anyhow};
use http_body_util::BodyExt as _;
use octocrab::{
    OctocrabBuilder,
    models::{
        StatusState, hooks,
        webhook_events::{WebhookEvent, WebhookEventPayload, WebhookEventType},
    },
};
use octocrab_extensions::OctocrabExt as _;
use poison_panic::MutexExt as _;
use rain_core::cache::persistent::PersistentCache;
use rain_lang::afs::{dir::Dir, entry::FSEntry, file::File, path::FilePath};
use rain_lang::driver::FSTrait as _;
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

                let download = self
                    .crab
                    .repos(&self.owner, &self.repo)
                    .download_tarball(head_commit.id.clone())
                    .await?;
                let download = download.into_body();
                let download = download.collect().await?.to_bytes();
                let ok = self.run(&download, &head_commit.id).await;
                let ci_status = if ok {
                    StatusState::Success
                } else {
                    StatusState::Failure
                };
                self.crab
                    .repos(&self.owner, &self.repo)
                    .create_status(head_commit.id.clone(), ci_status)
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

    #[expect(clippy::unwrap_used)]
    async fn run(&self, download: &[u8], git_ref: &str) -> bool {
        // Need to do this to satisfy static lifetime
        let download = download.to_vec();
        let download_dir_name = format!("{}-{}-{}", self.owner, self.repo, git_ref);
        tokio::task::spawn_blocking(move || Self::blocking_run(&download, &download_dir_name))
            .await
            .unwrap()
    }

    #[expect(
        clippy::unwrap_used,
        clippy::print_stdout,
        clippy::cognitive_complexity
    )]
    fn blocking_run(download: &[u8], download_dir_name: &str) -> bool {
        use rain_lang::driver::DriverTrait as _;
        let declaration = "ci";
        let config = rain_core::config::Config::new();
        let persistent_cache = PersistentCache::load(&config.cache_json_path()).unwrap();
        let cache = persistent_cache.into_cache(&config);
        let cache = rain_core::cache::Cache::new(cache);
        let mut ir = rain_lang::ir::Rir::new();
        let driver = rain_core::driver::DriverImpl::new(config);
        let download_area = driver.create_area(&[]).unwrap();
        let download_entry = FSEntry::new(download_area, FilePath::new("/download").unwrap());
        std::fs::write(driver.resolve_fs_entry(&download_entry), download).unwrap();
        let download = File::new_checked(&driver, download_entry).unwrap();
        let area = driver.extract_tar_gz(&download).unwrap();
        let download_dir_entry = FSEntry::new(area, FilePath::new(download_dir_name).unwrap());
        let root = Dir::new_checked(&driver, download_dir_entry).unwrap();
        let area = driver.create_area(&[&root]).unwrap();
        let root_entry = FSEntry::new(area, FilePath::new("/root.rain").unwrap());
        tracing::info!("Root entry {root_entry}");
        let root = File::new_checked(&driver, root_entry).unwrap();
        let src = driver.read_file(&root).unwrap();
        let module = rain_lang::ast::parser::parse_module(&src);
        let mid = ir.insert_module(root, src, module).unwrap();
        let main = ir.resolve_global_declaration(mid, declaration).unwrap();
        let mut runner = rain_lang::runner::Runner::new(&mut ir, &cache, &driver);
        tracing::info!("Running");
        let res = runner.evaluate_and_call(main);
        let persistent_cache = PersistentCache::from_cache(&cache.0.plock());
        persistent_cache
            .save(&driver.config.cache_json_path())
            .unwrap();
        match res {
            Ok(value) => {
                tracing::info!("Value {value}");
                true
            }
            Err(err) => {
                tracing::error!("{err:?}");
                let err = err.resolve_ir(&ir);
                println!("{err}");
                false
            }
        }
    }
}
