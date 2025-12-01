use anyhow::Result;
use rain_ci_common::{FinishedRun, Run, RunId};

pub trait StorageTrait: Send + Sync + 'static {
    async fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId>;
    async fn dequeued_run(&self, id: &RunId) -> Result<()>;
    async fn finished_run(&self, id: &RunId, finished: FinishedRun) -> Result<()>;
    async fn get_run(&self, id: &RunId) -> Result<Run>;
}

pub mod inner {
    use std::str::FromStr as _;

    use anyhow::{Context as _, Result};
    use chrono::{NaiveDateTime, TimeDelta, Utc};
    use rain_ci_common::{FinishedRun, RepoHost, Repository, Run, RunId, RunStatus};

    pub struct Storage {
        pub pool: sqlx::PgPool,
    }

    impl Storage {
        pub fn new(pool: sqlx::PgPool) -> Self {
            Self { pool }
        }
    }

    impl super::StorageTrait for Storage {
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
                host: String,
                owner: String,
                name: String,
                commit: String,
                created_at: NaiveDateTime,
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
    use rain_ci_common::RunId;

    #[derive(Default)]
    pub struct Storage {
        next_id: AtomicI64,
        db: Mutex<HashMap<i64, rain_ci_common::Run>>,
    }

    impl super::StorageTrait for Storage {
        async fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId> {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let mut conn = self.db.plock();
            conn.insert(id, run);
            Ok(RunId(id))
        }

        async fn dequeued_run(&self, id: &RunId) -> Result<()> {
            let mut conn = self.db.plock();
            let run = conn.get_mut(&id.0).unwrap();
            run.dequeued_at = Some(Utc::now());
            Ok(())
        }

        async fn finished_run(
            &self,
            id: &RunId,
            finished: rain_ci_common::FinishedRun,
        ) -> Result<()> {
            let mut conn = self.db.plock();
            let run = conn.get_mut(&id.0).unwrap();
            run.finished = Some(finished);
            Ok(())
        }

        async fn get_run(&self, id: &RunId) -> Result<rain_ci_common::Run> {
            let conn = self.db.plock();
            Ok(conn.get(&id.0).cloned().unwrap())
        }
    }
}
