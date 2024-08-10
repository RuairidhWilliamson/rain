use anyhow::Result;
use clap::Parser as _;

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

    let channel_url = if let Some(channel_url) = channel {
        channel_url
    } else {
        smee_rs::create_channel(source).await?
    };
    eprintln!("webhook url: {channel_url}");
    eprintln!("target url: {target}");
    let mut smee = smee_rs::Smee::new(channel_url, target)?;
    smee.start().await?;
    Ok(())
}
