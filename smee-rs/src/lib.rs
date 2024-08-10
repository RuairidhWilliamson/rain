use std::time::Duration;

use anyhow::{Context, Result};
use eventsource_client::{Client as _, Event, ReconnectOptions, SSE};
use futures::TryStreamExt;
use reqwest::{
    header::{HeaderMap, HeaderName},
    redirect::Policy,
};

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
                SSE::Connected(_) => eprintln!("connected"),
                SSE::Event(ev) => {
                    let Err(err) = self.handle_event(ev).await else {
                        continue;
                    };
                    eprintln!("Error handling event");
                    eprintln!("{err:#}");
                }
                SSE::Comment(comment) => eprintln!("comment {comment:?}"),
            }
        }
        Ok(())
    }

    async fn handle_event(&self, event: Event) -> Result<()> {
        match event.event_type.as_str() {
            "ready" => {
                eprintln!("ready");
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
                eprintln!("forwarding webhook of length {}", body.len());
                self.client
                    .post(self.target.clone())
                    .headers(headers)
                    .body(body)
                    .send()
                    .await?;
            }
            _ => {
                eprintln!("unknown event: {event:?}");
            }
        }
        Ok(())
    }
}
