use chrono::{DateTime, TimeDelta, Utc};
use postgres_types::{FromSql, ToSql};

#[derive(Debug, Clone, ToSql, FromSql, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[postgres(transparent)]
#[sqlx(transparent)]
pub struct RunId(pub i64);

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub owner: String,
    pub name: String,
}

impl Repository {
    pub fn repo_url(&self) -> String {
        format!(
            "https://github.com/{owner}/{name}",
            owner = self.owner,
            name = self.name,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Run {
    pub repository: Repository,
    pub source: RunSource,
    pub commit: String,
    pub created_at: DateTime<Utc>,
    pub dequeued_at: Option<DateTime<Utc>>,
    pub finished: Option<FinishedRun>,
}

impl Run {
    pub fn state(&self) -> RunState {
        if self.finished.is_some() {
            RunState::Finished
        } else if self.dequeued_at.is_some() {
            RunState::InProgress
        } else {
            RunState::Queued
        }
    }

    pub fn commit_url(&self) -> String {
        format!(
            "{repo_url}/commit/{commit}",
            repo_url = self.repository.repo_url(),
            commit = self.commit,
        )
    }
}

#[derive(Debug, Clone, ToSql, FromSql, sqlx::Type)]
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

#[derive(Debug, Clone)]
pub struct FinishedRun {
    pub finished_at: DateTime<Utc>,
    pub status: RunStatus,
    pub execution_time: TimeDelta,
    pub output: String,
}

#[derive(Debug, Clone, ToSql, FromSql, sqlx::Type)]
pub enum RunStatus {
    Success,
    Failure,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Success => "Success",
            Self::Failure => "Failure",
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RunState {
    Queued,
    InProgress,
    Finished,
}

impl std::fmt::Display for RunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
