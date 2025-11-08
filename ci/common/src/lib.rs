use std::path::PathBuf;

use chrono::NaiveDateTime;
use postgres_types::{FromSql, ToSql};

#[derive(Debug, serde::Deserialize)]
pub struct DbConfig {
    pub host: String,
    pub name: String,
    pub user: String,
    pub password_file: PathBuf,
}

#[derive(Debug, Clone, ToSql, FromSql)]
#[postgres(transparent)]
pub struct RunId(uuid::Uuid);

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Run {
    pub source: RunSource,
    pub create: NaiveDateTime,
}

#[derive(Debug, Clone, ToSql, FromSql)]
pub enum RunSource {
    Github,
}

impl std::fmt::Display for RunSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Github => f.write_str("Github"),
        }
    }
}
