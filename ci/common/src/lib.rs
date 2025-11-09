use chrono::{NaiveDateTime, TimeDelta};
use postgres_types::{FromSql, ToSql};

#[derive(Debug, Clone, ToSql, FromSql)]
#[postgres(transparent)]
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

#[derive(Debug, Clone)]
pub struct Run {
    pub repository: Repository,
    pub source: RunSource,
    pub created_at: NaiveDateTime,
    pub dequeued_at: Option<NaiveDateTime>,
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

#[derive(Debug, Clone)]
pub struct FinishedRun {
    pub finished_at: NaiveDateTime,
    pub status: RunStatus,
    pub execution_time: TimeDelta,
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

#[derive(Debug, Clone, Copy)]
pub enum RunState {
    Queued,
    InProgress,
    Finished,
}

pub fn time_delta_from_millis(millis: i64) -> TimeDelta {
    TimeDelta::milliseconds(millis)
}
