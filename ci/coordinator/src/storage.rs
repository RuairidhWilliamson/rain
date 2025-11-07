use anyhow::Result;
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, ToSql, FromSql)]
pub enum RunSource {
    Github,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Run {
    pub source: RunSource,
}

pub trait StorageTrait: Send + Sync {
    fn create_run(&self, run: Run) -> Result<uuid::Uuid>;
    fn get_run(&self, id: &uuid::Uuid) -> Result<Option<Run>>;
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
        fn create_run(&self, run: super::Run) -> Result<uuid::Uuid> {
            let id = uuid::Uuid::new_v4();
            let mut conn = self.db.plock();
            conn.execute(
                "INSERT INTO runs (id, source) VALUES ($1, $2)",
                &[&id, &run.source],
            )?;
            Ok(id)
        }

        fn get_run(&self, id: &uuid::Uuid) -> Result<Option<super::Run>> {
            let mut conn = self.db.plock();
            let Some(row) = conn.query_opt("SELECT source FROM runs WHERE id=$1", &[&id])? else {
                return Ok(None);
            };
            Ok(Some(super::Run {
                source: row.get("source"),
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
        db: Mutex<HashMap<uuid::Uuid, super::Run>>,
    }

    impl super::StorageTrait for Storage {
        fn create_run(&self, run: super::Run) -> Result<uuid::Uuid> {
            let id = uuid::Uuid::new_v4();
            let mut conn = self.db.plock();
            conn.insert(id, run);
            Ok(id)
        }

        fn get_run(&self, id: &uuid::Uuid) -> Result<Option<super::Run>> {
            let conn = self.db.plock();
            Ok(conn.get(id).cloned())
        }
    }
}
