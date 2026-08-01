mod forgejo;
mod github;
mod prepare;
mod repo_host;
mod runner;
mod server;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use alias::Alias as _;
use anyhow::Result;
use http::Request;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use ipnet::IpNet;
use rain_ci_common::db::{Db, DbConfig, run::RunId};
use runner::Runner;
use sqlx::postgres::PgListener;
use tokio::{sync::mpsc::Sender, task::JoinSet};
use tracing::{error, info, warn};
use url::Url;

#[derive(Debug, serde::Deserialize)]
struct Config {
    addr: SocketAddr,
    target_url: url::Url,
    seal: bool,
    database_url: Url,
    database_password_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();
    tracing_subscriber::fmt().init();
    if let Err(err) = dotenv_result {
        warn!(".env could not be loaded: {err:#}");
    }
    let config = envy::from_env::<Config>()?;
    let version = env!("CARGO_PKG_VERSION");
    info!("version = {version}");

    let allowed_ipnets: Option<&[IpNet]> = None;
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    let db = Db::new(
        DbConfig {
            url: config.database_url,
            password_file: config.database_password_file,
        },
        "rain-ci-coordinator",
    )
    .await?;

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    start_pg_notify_worker(&db, &tx);

    let server = Arc::new(server::Server {
        runner: Runner::new(config.seal),
        target_url: config.target_url,
        db,
        tx,
    });
    server.cleanup_old_runs().await?;
    server.start_server_run_request_worker(rx);
    info!("listening on http://{}", listener.local_addr()?);
    let mut join_set = JoinSet::new();
    loop {
        let (stream, addr) = listener.accept().await?;
        if let Some(allowed_ipnets) = allowed_ipnets
            && !allowed_ipnets
                .iter()
                .any(|ipnet| ipnet.contains(&addr.ip()))
        {
            warn!("connection {addr:?} did not match allowed ipnets");
            continue;
        }
        let server = server.alias();
        join_set.spawn(async move {
            let result = Builder::new(TokioExecutor::new())
                .serve_connection(
                    TokioIo::new(stream),
                    service_fn(|request: Request<Incoming>| {
                        let server = server.alias();
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

struct RunRequest {
    run_id: RunId,
}

fn start_pg_notify_worker(db: &Db, tx: &Sender<RunRequest>) {
    let db = db.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        if let Err(err) = pg_notify_worker(db, tx).await {
            error!("pg notify worker error: {err}");
        }
    });
}

async fn pg_notify_worker(db: Db, tx: Sender<RunRequest>) -> anyhow::Result<()> {
    let mut listener = PgListener::connect_with(&db.pool).await?;
    listener.listen("request_run").await?;
    loop {
        let notif = listener.recv().await?;
        assert_eq!(notif.channel(), "request_run");
        let run_id: i64 = notif.payload().parse()?;
        tx.send(RunRequest {
            run_id: RunId(run_id),
        })
        .await?;
    }
}
