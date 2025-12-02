mod runner;
mod server;

mod storage;
#[cfg(test)]
mod tests;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context as _, Result};
use http::Request;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use ipnet::IpNet;
use jsonwebtoken::EncodingKey;
use log::{error, info, warn};
use rain_ci_common::RunId;
use runner::Runner;
use secrecy::{ExposeSecret as _, SecretString};
use sqlx::postgres::PgListener;
use tokio::task::JoinSet;

use crate::storage::StorageTrait as _;

#[derive(Debug, serde::Deserialize)]
struct Config {
    addr: SocketAddr,
    github_app_id: rain_ci_common::github::model::AppId,
    github_app_key_file: PathBuf,
    github_webhook_secret: String,
    target_url: url::Url,
    seal: bool,
    db_host: String,
    db_name: String,
    db_user: String,
    db_password_file: Option<PathBuf>,
    db_password: Option<SecretString>,
}

async fn load_password(config: &Config) -> Result<SecretString> {
    if let Some(password) = &config.db_password {
        return Ok(password.clone());
    }
    if let Some(password_file) = &config.db_password_file {
        return Ok(tokio::fs::read_to_string(password_file)
            .await
            .context("cannot read DB_PASSWORD_FILE")?
            .into());
    }
    Err(anyhow::anyhow!("set DB_PASSWORD or DB_PASSWORD_FILE"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();
    env_logger::init();
    if let Err(err) = dotenv_result {
        warn!(".env could not be loaded: {err:#}");
    }
    let config = envy::from_env::<Config>()?;
    let version = env!("CARGO_PKG_VERSION");
    info!("version = {version}");

    let key_raw = secrecy::SecretSlice::from(
        tokio::fs::read(&config.github_app_key_file)
            .await
            .context("read github app key")?,
    );
    let key =
        EncodingKey::from_rsa_pem(key_raw.expose_secret()).context("decode github app key")?;

    let github_client = rain_ci_common::github::implementation::AppClient::new(
        rain_ci_common::github::implementation::AppAuth {
            app_id: config.github_app_id,
            key,
        },
    );

    // let ipnets = [IpNet::from(IpAddr::V4(Ipv4Addr::LOCALHOST))];
    // let mut allowed_ipnets = Some(&ipnets);
    let allowed_ipnets: Option<&[IpNet]> = None;
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    let db_password = load_password(&config).await?;
    let pool = sqlx::postgres::PgPool::connect_with(
        sqlx::postgres::PgConnectOptions::new()
            .host(&config.db_host)
            .username(&config.db_user)
            .password(db_password.expose_secret())
            .database(&config.db_name),
    )
    .await?;

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    {
        let pool = pool.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut listener = PgListener::connect_with(&pool).await.unwrap();
            listener.listen("request_run").await.unwrap();
            loop {
                let notif = listener.recv().await.unwrap();
                assert_eq!(notif.channel(), "request_run");
                let run_id: i64 = notif.payload().parse().unwrap();
                tx.send(RunRequest {
                    run_id: RunId(run_id),
                })
                .await
                .unwrap();
            }
        });
    }

    let storage = storage::inner::Storage::new(pool);
    cleanup_old_runs(&storage).await?;

    let server = Arc::new(server::Server {
        runner: Runner::new(config.seal),
        github_webhook_secret: config.github_webhook_secret,
        target_url: config.target_url,
        github_client,
        storage,
        tx,
    });
    {
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            loop {
                let Some(check_suite_event) = rx.recv().await else {
                    error!("server recv channel closed");
                    return;
                };
                if let Err(err) = Arc::clone(&server)
                    .handle_run_request(check_suite_event)
                    .await
                {
                    error!("handle check suite event: {err}");
                }
            }
        });
    }
    info!("listening on {}", config.addr);
    let mut join_set = JoinSet::new();
    loop {
        let (stream, addr) = listener.accept().await?;
        if let Some(allowed_ipnets) = allowed_ipnets {
            if !allowed_ipnets
                .iter()
                .any(|ipnet| ipnet.contains(&addr.ip()))
            {
                warn!("connection {addr:?} did not match allowed ipnets");
                continue;
            }
        }
        let server = Arc::clone(&server);
        join_set.spawn(async move {
            let result = Builder::new(TokioExecutor::new())
                .serve_connection(
                    TokioIo::new(stream),
                    service_fn(|request: Request<Incoming>| {
                        let server = Arc::clone(&server);
                        async move { server::Server::handle_request(server, request).await }
                    }),
                )
                .await;

            if let Err(err) = result {
                error!("serve connection: {err:#}");
            }
        });
    }
}

async fn cleanup_old_runs(storage: &storage::inner::Storage) -> Result<()> {
    let ids = sqlx::query!("SELECT id FROM runs LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run WHERE dequeued_at IS NOT NULL AND run IS NULL")
        .fetch_all(&storage.pool)
        .await?;
    for row in ids {
        storage
            .finished_run(
                &rain_ci_common::RunId(row.id),
                rain_ci_common::FinishedRun {
                    finished_at: chrono::Utc::now(),
                    status: rain_ci_common::RunStatus::SystemFailure,
                    execution_time: chrono::TimeDelta::zero(),
                    output: String::from("run was cleaned up on coordinator startup"),
                },
            )
            .await?;
    }
    Ok(())
}

struct RunRequest {
    run_id: RunId,
}
