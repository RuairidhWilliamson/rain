use anyhow::Result;
use rain_ci_common::{FinishedRun, RunId};

pub trait StorageTrait: Send + Sync + 'static {
    async fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId>;
    async fn dequeued_run(&self, id: &RunId) -> Result<()>;
    async fn finished_run(&self, id: &RunId, finished: FinishedRun) -> Result<()>;
}

pub mod inner {
    use anyhow::Result;
    use chrono::Utc;
    use rain_ci_common::{FinishedRun, RunId};

    pub struct Storage {
        pool: sqlx::PgPool,
    }

    impl Storage {
        pub fn new(pool: sqlx::PgPool) -> Self {
            Self { pool }
        }
    }

    impl super::StorageTrait for Storage {
        async fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId> {
            let source: &str = run.source.into();
            let row = sqlx::query!("INSERT INTO runs (source, created_at, repo_owner, repo_name, commit) VALUES ($1, $2, $3, $4, $5) RETURNING id", source, run.created_at.naive_utc(), &run.repository.owner, &run.repository.name, &run.commit).fetch_one(&self.pool).await?;
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
    }
}
