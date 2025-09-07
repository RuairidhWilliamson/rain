use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum RunSource {
    Github,
}

#[derive(Serialize, Deserialize)]
pub struct Run {
    pub source: RunSource,
}

pub struct Storage {
    pub db: rocksdb::DB,
}

impl Storage {
    pub fn insert_run(&self, run: &Run) -> Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        self.db
            .put(format!("run.{id}"), postcard::to_allocvec(&run)?)?;
        Ok(id)
    }

    #[expect(dead_code)]
    pub fn get_run(&self, id: uuid::Uuid) -> Result<Run> {
        let bytes = self
            .db
            .get_pinned(format!("run.{id}"))?
            .context("does not exist")?;
        Ok(postcard::from_bytes(&bytes)?)
    }
}
