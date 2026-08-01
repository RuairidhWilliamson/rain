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
use tokio::{
    sync::{Mutex, mpsc::Sender},
    task::JoinSet,
};
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

    let (run_request, run_request_rx) = tokio::sync::mpsc::channel(10);
    start_run_request_pg_notify_worker(&db, &run_request);
    let (cancel_run, cancel_run_rx) = tokio::sync::mpsc::channel(10);
    start_cancel_run_pg_notify_worker(&db, &cancel_run);

    let server = Arc::new(server::Server {
        runner: Runner::new(config.seal),
        target_url: config.target_url,
        db,
        run_request,
        active_run: Mutex::default(),
    });
    server.cleanup_old_runs().await?;

    server.start_server_run_request_worker(run_request_rx);

    server.start_server_cancel_run_worker(cancel_run_rx);
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

struct CancelRun {
    run_id: RunId,
}

fn start_run_request_pg_notify_worker(db: &Db, run_request: &Sender<RunRequest>) {
    async fn worker(db: Db, tx: Sender<RunRequest>) -> anyhow::Result<()> {
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
    let db = db.clone();
    let tx = run_request.clone();
    tokio::spawn(async move {
        if let Err(err) = worker(db, tx).await {
            error!("pg notify worker error: {err}");
        }
    });
}

fn start_cancel_run_pg_notify_worker(db: &Db, cancel_run: &Sender<CancelRun>) {
    async fn worker(db: Db, tx: Sender<CancelRun>) -> anyhow::Result<()> {
        let mut listener = PgListener::connect_with(&db.pool).await?;
        listener.listen("cancel_run").await?;
        loop {
            let notif = listener.recv().await?;
            assert_eq!(notif.channel(), "cancel_run");
            let run_id: i64 = notif.payload().parse()?;
            tx.send(CancelRun {
                run_id: RunId(run_id),
            })
            .await?;
        }
    }
    let db = db.clone();
    let tx = cancel_run.clone();
    tokio::spawn(async move {
        if let Err(err) = worker(db, tx).await {
            error!("pg notify worker error: {err}");
        }
    });
}
