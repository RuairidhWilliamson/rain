pub mod github;

use std::str::FromStr as _;

use anyhow::Context as _;
use chrono::{DateTime, TimeDelta, Utc};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunId(pub i64);

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct RepositoryId(pub i64);

impl std::fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub id: RepositoryId,
    pub host: RepoHost,
    pub owner: String,
    pub name: String,
}

impl Repository {
    pub fn external_repo_url(&self) -> String {
        match self.host.api() {
            RepoHostAPI::Github | RepoHostAPI::Gitlab | RepoHostAPI::Forgejo => format!(
                "{url}/{owner}/{name}",
                url = self.host.url(),
                owner = self.owner,
                name = self.name,
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Run {
    pub repository: Repository,
    pub commit: String,
    pub created_at: DateTime<Utc>,
    pub dequeued_at: Option<DateTime<Utc>>,
    pub finished: Option<FinishedRun>,
    pub target: String,
    pub rain_version: Option<String>,
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
            repo_url = self.repository.external_repo_url(),
            commit = self.commit,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoHost {
    Github,
    Gitlab,
    Codeberg,
    Custom(RepoHostAPI, String),
}

impl std::fmt::Display for RepoHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Github => f.write_str("Github"),
            Self::Gitlab => f.write_str("Gitlab"),
            Self::Codeberg => f.write_str("Codeberg"),
            Self::Custom(_, url) => f.write_str(url),
        }
    }
}

impl RepoHost {
    pub fn new(api: &str, url: &str) -> anyhow::Result<Self> {
        Ok(match url {
            "https://github.com" => Self::Github,
            url => Self::Custom(
                RepoHostAPI::from_str(api).context("unknown repo host api")?,
                url.to_owned(),
            ),
        })
    }

    pub fn api(&self) -> RepoHostAPI {
        match self {
            Self::Github => RepoHostAPI::Github,
            Self::Gitlab => RepoHostAPI::Gitlab,
            Self::Codeberg => RepoHostAPI::Forgejo,
            Self::Custom(repo_host_api, _) => *repo_host_api,
        }
    }

    pub fn url(&self) -> &str {
        match self {
            Self::Github => "https://github.com",
            Self::Gitlab => "https://gitlab.com",
            Self::Codeberg => "https://codeberg.org",
            Self::Custom(_, url) => url,
        }
    }
}

#[derive(Debug, Clone, Copy, strum::IntoStaticStr, strum::EnumString, PartialEq, Eq)]
pub enum RepoHostAPI {
    Github,
    Gitlab,
    Forgejo,
}

#[derive(Debug, Clone)]
pub struct FinishedRun {
    pub finished_at: DateTime<Utc>,
    pub status: RunStatus,
    pub execution_time: TimeDelta,
    pub output: String,
}

#[derive(Debug, Clone, strum::IntoStaticStr, strum::EnumString)]
pub enum RunStatus {
    Success,
    Failure,
    SystemFailure,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.into())
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
