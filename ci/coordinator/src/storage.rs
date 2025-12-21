use anyhow::Result;
use rain_ci_common::{FinishedRun, RepoHost, RepositoryId, Run, RunId};

pub trait StorageTrait: Send + Sync + 'static {
    async fn create_or_get_repo(
        &self,
        host: &RepoHost,
        owner: &str,
        name: &str,
    ) -> Result<RepositoryId>;
    async fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId>;
    fn dequeued_run(&self, id: &RunId) -> impl std::future::Future<Output = Result<()>> + Send;
    fn finished_run(
        &self,
        id: &RunId,
        finished: FinishedRun,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_run(&self, id: &RunId) -> impl std::future::Future<Output = Result<Run>> + Send;
}

pub mod inner {
    use std::str::FromStr as _;

    use anyhow::{Context as _, Result};
    use chrono::{NaiveDateTime, TimeDelta, Utc};
    use rain_ci_common::{FinishedRun, RepoHost, Repository, RepositoryId, Run, RunId, RunStatus};

    pub struct Storage {
        pub pool: sqlx::PgPool,
    }

    impl Storage {
        pub fn new(pool: sqlx::PgPool) -> Self {
            Self { pool }
        }
    }

    impl super::StorageTrait for Storage {
        async fn create_or_get_repo(
            &self,
            host: &RepoHost,
            owner: &str,
            name: &str,
        ) -> Result<rain_ci_common::RepositoryId> {
            let host: &str = host.into();
            let mut tx = self.pool.begin().await?;
            if let Some(row) = sqlx::query!(
                "SELECT id FROM repos WHERE host=$1 AND owner=$2 AND name=$3",
                host,
                owner,
                name
            )
            .fetch_optional(&mut *tx)
            .await?
            {
                tx.commit().await?;
                return Ok(RepositoryId(row.id));
            }
            let row = sqlx::query!(
                "INSERT INTO repos (host, owner, name) VALUES ($1, $2, $3) RETURNING id",
                host,
                owner,
                name
            )
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(RepositoryId(row.id))
        }

        async fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId> {
            let host: &str = run.repository.host.into();
            let mut tx = self.pool.begin().await?;
            let repo = sqlx::query!(
                "SELECT id FROM repos WHERE host=$1 AND owner=$2 AND name=$3",
                host,
                &run.repository.owner,
                &run.repository.name
            )
            .fetch_one(&mut *tx)
            .await?;
            let row = sqlx::query!(
                "INSERT INTO runs (created_at, repo, commit) VALUES ($1, $2, $3) RETURNING id",
                run.created_at.naive_utc(),
                repo.id,
                &run.commit
            )
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(RunId(row.id))
        }

        async fn dequeued_run(&self, id: &RunId) -> Result<()> {
            sqlx::query!(
                "UPDATE runs SET dequeued_at=$1 WHERE id=$2",
                &Utc::now().naive_utc(),
                id.0,
            )
            .execute(&self.pool)
            .await?;
            Ok(())
        }

        async fn finished_run(&self, id: &RunId, finished: FinishedRun) -> Result<()> {
            let run_status: &str = finished.status.into();
            sqlx::query!("INSERT INTO finished_runs (run, finished_at, status, execution_time_millis, output) VALUES ($1, $2, $3, $4, $5)", id.0, finished.finished_at.naive_utc(), run_status, finished.execution_time.num_milliseconds(), &finished.output).execute(&self.pool).await?;
            Ok(())
        }

        async fn get_run(&self, id: &RunId) -> Result<Run> {
            struct QueryRun {
                repo_id: i64,
                host: String,
                owner: String,
                name: String,
                commit: String,
                created_at: NaiveDateTime,
                target: String,
                dequeued_at: Option<NaiveDateTime>,
                status: Option<String>,
                finished_at: Option<NaiveDateTime>,
                execution_time_millis: Option<i64>,
                output: Option<String>,
            }
            let row = sqlx::query_file_as!(QueryRun, "queries/get_run.sql", id.0)
                .fetch_one(&self.pool)
                .await?;
            Ok(Run {
                commit: row.commit,
                created_at: row.created_at.and_utc(),
                dequeued_at: row.dequeued_at.map(|dt| dt.and_utc()),
                target: row.target,
                finished: row
                    .finished_at
                    .map(|finished_at| {
                        Result::<_>::Ok(rain_ci_common::FinishedRun {
                            finished_at: finished_at.and_utc(),
                            status: RunStatus::from_str(&row.status.context("status missing")?)
                                .context("unknown status")?,
                            execution_time: TimeDelta::milliseconds(
                                row.execution_time_millis
                                    .context("execution_time_millis missing")?,
                            ),
                            output: row.output.context("output missing")?,
                        })
                    })
                    .transpose()?,
                repository: Repository {
                    id: RepositoryId(row.repo_id),
                    host: RepoHost::from_str(&row.host).context("unknown repo host")?,
                    owner: row.owner,
                    name: row.name,
                },
            })
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::{
        collections::HashMap,
        sync::{
            Mutex,
            atomic::{AtomicI64, Ordering},
        },
    };

    use anyhow::Result;
    use chrono::Utc;
    use poison_panic::MutexExt as _;
    use rain_ci_common::{RepositoryId, RunId};

    #[derive(Default)]
    pub struct Storage {
        repos: RepoStorage,
        runs: RunStorage,
    }

    #[derive(Default)]
    struct RepoStorage {
        next_id: AtomicI64,
        db: Mutex<HashMap<i64, rain_ci_common::Repository>>,
    }

    #[derive(Default)]
    struct RunStorage {
        next_id: AtomicI64,
        db: Mutex<HashMap<i64, rain_ci_common::Run>>,
    }

    impl super::StorageTrait for Storage {
        async fn create_or_get_repo(
            &self,
            host: &rain_ci_common::RepoHost,
            owner: &str,
            name: &str,
        ) -> Result<rain_ci_common::RepositoryId> {
            let mut repos = self.repos.db.plock();
            if let Some((repo_id, _)) = repos
                .iter()
                .find(|(_, repo)| &repo.host == host && repo.owner == owner && repo.name == name)
            {
                Ok(RepositoryId(*repo_id))
            } else {
                let id = RepositoryId(self.repos.next_id.fetch_add(1, Ordering::Relaxed));
                repos.insert(
                    id.0,
                    rain_ci_common::Repository {
                        id: id,
                        host: host.clone(),
                        owner: owner.to_owned(),
                        name: name.to_owned(),
                    },
                );
                Ok(id)
            }
        }

        async fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId> {
            let id = self.runs.next_id.fetch_add(1, Ordering::Relaxed);
            let mut conn = self.runs.db.plock();
            conn.insert(id, run);
            Ok(RunId(id))
        }

        async fn dequeued_run(&self, id: &RunId) -> Result<()> {
            let mut conn = self.runs.db.plock();
            let run = conn.get_mut(&id.0).unwrap();
            run.dequeued_at = Some(Utc::now());
            Ok(())
        }

        async fn finished_run(
            &self,
            id: &RunId,
            finished: rain_ci_common::FinishedRun,
        ) -> Result<()> {
            let mut conn = self.runs.db.plock();
            let run = conn.get_mut(&id.0).unwrap();
            run.finished = Some(finished);
            Ok(())
        }

        async fn get_run(&self, id: &RunId) -> Result<rain_ci_common::Run> {
            let conn = self.runs.db.plock();
            Ok(conn.get(&id.0).cloned().unwrap())
        }
    }
}
