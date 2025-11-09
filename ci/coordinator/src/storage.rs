use anyhow::Result;

pub trait StorageTrait: Send + Sync {
    fn create_run(&self, run: rain_ci_common::Run) -> Result<uuid::Uuid>;
    #[expect(dead_code)]
    fn get_run(&self, id: &uuid::Uuid) -> Result<Option<rain_ci_common::Run>>;
}

pub mod inner {
    use std::sync::Mutex;

    use anyhow::Result;
    use poison_panic::MutexExt as _;

    pub struct Storage {
        db: Mutex<postgres::Client>,
    }

    impl Storage {
        pub fn new(db: postgres::Client) -> Self {
            Self { db: Mutex::new(db) }
        }
    }

    impl super::StorageTrait for Storage {
        fn create_run(&self, run: rain_ci_common::Run) -> Result<uuid::Uuid> {
            let id = uuid::Uuid::new_v4();
            let mut conn = self.db.plock();
            conn.execute(
                "INSERT INTO runs (id, source, created_at, state) VALUES ($1, $2, $3, $4)",
                &[&id, &run.source, &run.created_at, &run.state],
            )?;
            Ok(id)
        }

        fn get_run(&self, id: &uuid::Uuid) -> Result<Option<rain_ci_common::Run>> {
            let mut conn = self.db.plock();
            let Some(row) =
                conn.query_opt("SELECT source, created_at FROM runs WHERE id=$1", &[&id])?
            else {
                return Ok(None);
            };
            Ok(Some(rain_ci_common::Run {
                source: row.get("source"),
                created_at: row.get("created_at"),
                state: row.get("state"),
                status: row.get("status"),
            }))
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::{collections::HashMap, sync::Mutex};

    use anyhow::Result;
    use poison_panic::MutexExt as _;

    #[derive(Default)]
    pub struct Storage {
        db: Mutex<HashMap<uuid::Uuid, rain_ci_common::Run>>,
    }

    impl super::StorageTrait for Storage {
        fn create_run(&self, run: rain_ci_common::Run) -> Result<uuid::Uuid> {
            let id = uuid::Uuid::new_v4();
            let mut conn = self.db.plock();
            conn.insert(id, run);
            Ok(id)
        }

        fn get_run(&self, id: &uuid::Uuid) -> Result<Option<rain_ci_common::Run>> {
            let conn = self.db.plock();
            Ok(conn.get(id).cloned())
        }
    }
}
