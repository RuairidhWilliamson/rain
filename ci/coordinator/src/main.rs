mod github;
mod runner;
mod server;

mod storage;
#[cfg(test)]
mod tests;

use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context as _, Result};
use ipnet::IpNet;
use jsonwebtoken::EncodingKey;
use log::{error, info, warn};
use postgres::NoTls;
use runner::Runner;

#[derive(Debug, serde::Deserialize)]
struct Config {
    addr: SocketAddr,
    github_app_id: github::model::AppId,
    github_app_key: PathBuf,
    github_webhook_secret: String,
    target_url: url::Url,
    seal: bool,
    db_host: String,
    db_name: String,
    db_user: String,
    db_password_file: PathBuf,
}

fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();
    env_logger::init();
    if let Err(err) = dotenv_result {
        warn!(".env could not be loaded: {err:#}");
    }
    let config = envy::from_env::<Config>()?;
    let version = env!("CARGO_PKG_VERSION");
    info!("version = {version}");

    let key_raw = std::fs::read(&config.github_app_key).context("read github app key")?;
    let key = EncodingKey::from_rsa_pem(&key_raw).context("decode github app key")?;

    let github_client = github::implementation::AppClient::new(github::implementation::AppAuth {
        app_id: config.github_app_id,
        key,
    });

    // let ipnets = [IpNet::from(IpAddr::V4(Ipv4Addr::LOCALHOST))];
    // let mut allowed_ipnets = Some(&ipnets);
    let allowed_ipnets: Option<&[IpNet]> = None;
    let listener = std::net::TcpListener::bind(config.addr)?;
    let db = postgres::Config::new()
        .host(&config.db_host)
        .dbname(&config.db_name)
        .user(&config.db_user)
        .password(std::fs::read_to_string(config.db_password_file)?)
        .connect(NoTls)?;
    let server = server::Server {
        runner: Runner::new(config.seal),
        github_webhook_secret: config.github_webhook_secret,
        target_url: config.target_url,
        github_client,
        storage: Box::new(storage::inner::Storage::new(db)),
    };
    loop {
        let (stream, addr) = listener.accept()?;
        if let Some(allowed_ipnets) = allowed_ipnets {
            if !allowed_ipnets
                .iter()
                .any(|ipnet| ipnet.contains(&addr.ip()))
            {
                warn!("connection {addr:?} did not match allowed ipnets");
                continue;
            }
        }
        if let Err(err) = server.handle_connection(stream, addr) {
            error!("handle connection: {err:#}");
        }
    }
}
