use std::time::Duration;

use anyhow::{Context as _, Result};
use eventsource_client::{Client as _, Event, ReconnectOptions, SSE};
use futures::TryStreamExt as _;
use reqwest::{
    header::{HeaderMap, HeaderName},
    redirect::Policy,
};

pub const DEFAULT_SMEE_SERVER: &str = "https://smee.io/new";

/// Returns the default smee server url
pub fn default_smee_server_url() -> url::Url {
    let Ok(url) = url::Url::parse(DEFAULT_SMEE_SERVER) else {
        unreachable!()
    };
    url
}

pub struct Channel<H> {
    channel: url::Url,
    handler: H,
}

impl<H> Channel<H> {
    /// Create a channel using a smee server
    ///
    /// `smee_server` is the url of the remote server to use e.g. `https://smee.io/new`
    pub async fn new(smee_server: url::Url, handler: H) -> Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .redirect(Policy::none())
            .build()?;
        let response = client.head(smee_server).send().await?.error_for_status()?;
        let url = response
            .headers()
            .get("location")
            .context("no redirect location")?;
        let channel = url.to_str()?.try_into()?;
        Ok(Self { channel, handler })
    }

    pub fn from_existing_channel(channel: url::Url, handler: H) -> Self {
        Self { channel, handler }
    }

    pub fn get_channel_url(&self) -> &url::Url {
        &self.channel
    }
}

impl<H: MessageHandler> Channel<H> {
    pub async fn start(&mut self) -> Result<()> {
        let se_client = eventsource_client::ClientBuilder::for_url(self.channel.as_str())?
            .reconnect(
                ReconnectOptions::reconnect(true)
                    .retry_initial(false)
                    .delay(Duration::from_secs(1))
                    .backoff_factor(2)
                    .delay_max(Duration::from_secs(60))
                    .build(),
            )
            .build();
        let mut stream = se_client.stream();
        while let Some(event) = stream.try_next().await? {
            match event {
                SSE::Connected(_) => log::info!("connected"),
                SSE::Event(ev) => {
                    let Err(err) = Box::pin(self.handle_event(ev)).await else {
                        continue;
                    };
                    log::error!("Error handling event");
                    log::error!("{err:#}");
                }
                SSE::Comment(comment) => log::info!("comment {comment:?}"),
            }
        }
        Ok(())
    }

    async fn handle_event(&self, event: Event) -> Result<()> {
        match event.event_type.as_str() {
            "ready" => {
                log::info!("ready");
            }
            "ping" => {}
            "message" => {
                let data: serde_json::Value = serde_json::from_str(&event.data)?;
                let h = data.as_object().context("data is not object")?;
                let body = data.get("body").context("no body")?.to_string();
                let mut headers = HeaderMap::new();
                for (k, v) in h {
                    if let Some(v) = v.as_str() {
                        headers.insert(k.parse::<HeaderName>()?, v.parse()?);
                    }
                }
                self.handler.handle(headers, body).await?;
            }
            _ => {
                log::error!("unknown event: {event:?}");
            }
        }
        Ok(())
    }
}

pub trait MessageHandler: Send + Sync {
    #[expect(async_fn_in_trait)]
    async fn handle(&self, headers: HeaderMap, body: String) -> Result<()>;
}

pub struct ForwardHandler {
    pub client: reqwest::Client,
    pub target: url::Url,
}

impl ForwardHandler {
    pub fn new(target: url::Url) -> Self {
        Self {
            client: reqwest::Client::new(),
            target,
        }
    }
}

impl MessageHandler for ForwardHandler {
    async fn handle(&self, headers: HeaderMap, body: String) -> Result<()> {
        log::info!("forwarding webhook of length {}", body.len());
        self.client
            .post(self.target.clone())
            .headers(headers)
            .body(body)
            .send()
            .await?;
        Ok(())
    }
}

pub async fn create_channel(url: url::Url) -> Result<url::Url> {
    let client = reqwest::ClientBuilder::new()
        .redirect(Policy::none())
        .build()?;
    let response = client.head(url).send().await?.error_for_status()?;
    let url = response
        .headers()
        .get("location")
        .context("no redirect location")?;
    Ok(url.to_str()?.try_into()?)
}

pub struct Smee {
    client: reqwest::Client,
    source: url::Url,
    target: url::Url,
}

impl Smee {
    pub fn new(channel_url: url::Url, target: url::Url) -> Result<Self> {
        let client = reqwest::ClientBuilder::new().build()?;
        Ok(Self {
            client,
            source: channel_url,
            target,
        })
    }

    pub async fn start(&self) -> Result<()> {
        let se_client = eventsource_client::ClientBuilder::for_url(self.source.as_str())?
            .reconnect(
                ReconnectOptions::reconnect(true)
                    .retry_initial(false)
                    .delay(Duration::from_secs(1))
                    .backoff_factor(2)
                    .delay_max(Duration::from_secs(60))
                    .build(),
            )
            .build();
        let mut stream = se_client.stream();
        while let Some(event) = stream.try_next().await? {
            match event {
                SSE::Connected(_) => log::info!("connected"),
                SSE::Event(ev) => {
                    let Err(err) = self.handle_event(ev).await else {
                        continue;
                    };
                    log::error!("Error handling event");
                    log::error!("{err:#}");
                }
                SSE::Comment(comment) => log::info!("comment {comment:?}"),
            }
        }
        Ok(())
    }

    async fn handle_event(&self, event: Event) -> Result<()> {
        match event.event_type.as_str() {
            "ready" => {
                log::info!("ready");
            }
            "ping" => {}
            "message" => {
                let data: serde_json::Value = serde_json::from_str(&event.data)?;
                let h = data.as_object().context("data is not object")?;
                let body = data.get("body").context("no body")?.to_string();
                let mut headers = HeaderMap::new();
                for (k, v) in h {
                    if let Some(v) = v.as_str() {
                        headers.insert(k.parse::<HeaderName>()?, v.parse()?);
                    }
                }
                log::info!("forwarding webhook of length {}", body.len());
                self.client
                    .post(self.target.clone())
                    .headers(headers)
                    .body(body)
                    .send()
                    .await?;
            }
            _ => {
                log::error!("unknown event: {event:?}");
            }
        }
        Ok(())
    }
}
