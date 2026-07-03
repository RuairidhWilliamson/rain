use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use http::Request;
use hyper::body::Incoming;
use rain_ci_common::db::{
    WithId,
    repository::Repository,
    run::{Run, RunStatus},
};

use crate::server::Server;

pub trait RepoHostApi {
    fn handle_webhook(
        &self,
        server: &Server,
        request: Request<Incoming>,
    ) -> impl Future<Output = Result<()>>;
    fn handle_run_request(
        &self,
        server: Arc<Server>,
        run: WithId<Run>,
        repository: WithId<Repository>,
        start: chrono::DateTime<Utc>,
    ) -> impl Future<Output = Result<()>>;
    #[expect(clippy::too_many_arguments)]
    fn finish_run(
        &self,
        server: &Arc<Server>,
        run: WithId<Run>,
        repository: WithId<Repository>,
        status: RunStatus,
        output: String,
        finished_at: chrono::DateTime<Utc>,
        execution_time: chrono::TimeDelta,
    ) -> impl Future<Output = Result<()>>;
}
