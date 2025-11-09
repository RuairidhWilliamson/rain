use chrono::NaiveDateTime;
use postgres_types::{FromSql, ToSql};

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
    pub created_at: NaiveDateTime,
    pub state: RunState,
    pub status: Option<RunStatus>,
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

#[derive(Debug, Clone, ToSql, FromSql)]
pub enum RunState {
    Queued,
    InProgress,
    Finished,
}

impl std::fmt::Display for RunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Queued => "Queued",
            Self::InProgress => "Running",
            Self::Finished => "Finished",
        })
    }
}

#[derive(Debug, Clone, ToSql, FromSql)]
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
