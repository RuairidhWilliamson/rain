use anyhow::Result;
use rain_ci_common::{FinishedRun, RunId};

pub trait StorageTrait: Send + Sync {
    fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId>;
    fn dequeued_run(&self, id: &RunId) -> Result<()>;
    fn finished_run(&self, id: &RunId, finished: FinishedRun) -> Result<()>;
}

pub mod inner {
    use std::sync::Mutex;

    use anyhow::Result;
    use chrono::Utc;
    use poison_panic::MutexExt as _;
    use rain_ci_common::{FinishedRun, RunId};

    pub struct Storage {
        // TODO: Use connection pool instead of just one connection 🤦
        db: Mutex<postgres::Client>,
    }

    impl Storage {
        pub fn new(db: postgres::Client) -> Self {
            Self { db: Mutex::new(db) }
        }
    }

    impl super::StorageTrait for Storage {
        fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId> {
            let mut conn = self.db.plock();
            let row = conn.query_one(
                "INSERT INTO runs (source, created_at, repo_owner, repo_name) VALUES ($1, $2, $3, $4) RETURNING id",
                &[&run.source, &run.created_at, &run.repository.owner, &run.repository.name],
            )?;
            Ok(RunId(row.get("id")))
        }

        fn dequeued_run(&self, id: &RunId) -> Result<()> {
            let mut conn = self.db.plock();
            conn.execute(
                "UPDATE runs SET dequeued_at=$1 WHRE id=$2",
                &[&Utc::now().naive_utc(), id],
            )?;
            Ok(())
        }

        fn finished_run(&self, id: &RunId, finished: FinishedRun) -> Result<()> {
            let mut conn = self.db.plock();
            let mut tx = conn.transaction()?;
            let row = tx.query_one("INSERT INTO finished_runs (finished_at, status, execution_time_millis) VALUES ($1, $2, $3) RETURNING id", &[&finished.finished_at, &finished.status, &finished.execution_time.num_milliseconds()])?;
            let finished_id: i64 = row.get("id");
            tx.execute(
                "UPDATE runs SET finished=$1 WHRE id=$2",
                &[&finished_id, id],
            )?;
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
        fn create_run(&self, run: rain_ci_common::Run) -> Result<RunId> {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let mut conn = self.db.plock();
            conn.insert(id, run);
            Ok(RunId(id))
        }

        fn dequeued_run(&self, id: &RunId) -> Result<()> {
            let mut conn = self.db.plock();
            let run = conn.get_mut(&id.0).unwrap();
            run.dequeued_at = Some(Utc::now().naive_utc());
            Ok(())
        }

        fn finished_run(&self, id: &RunId, finished: rain_ci_common::FinishedRun) -> Result<()> {
            let mut conn = self.db.plock();
            let run = conn.get_mut(&id.0).unwrap();
            run.finished = Some(finished);
            Ok(())
        }
    }
}
