use anyhow::Result;
use clap::Parser as _;
use smee_rs::{Channel, ForwardHandler};

#[derive(clap::Parser)]
struct Cli {
    #[arg(short, long)]
    channel: Option<url::Url>,

    #[arg(short, long, default_value = "https://smee.io/new")]
    source: url::Url,

    #[arg(short, long, default_value = "http://127.0.0.1:3000")]
    target: url::Url,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Cli {
        channel,
        source,
        target,
    } = Cli::parse();

    eprintln!("target url: {target}");
    let handler = ForwardHandler::new(target);
    let mut channel = if let Some(channel_url) = channel {
        Channel::from_existing_channel(channel_url, handler)
    } else {
        Channel::new(source, handler).await?
    };
    eprintln!("webhook url: {}", channel.get_channel_url());
    eprintln!("listening...");
    channel.start().await?;
    Ok(())
}
