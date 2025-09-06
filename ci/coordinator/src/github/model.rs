use std::fmt::Display;

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AppId(u64);

impl Display for AppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InstallationId(pub u64);

impl Display for InstallationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiOverview {
    pub hooks: Vec<IpNet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSuiteEvent {
    pub sender: User,
    pub repository: Repository,
    pub installation: SimpleInstallation,
    pub action: Action,
    pub check_suite: CheckSuite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Requested,
    Rerequested,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSuite {
    pub id: i64,
    pub created_at: String,
    pub head_sha: String,
    pub head_branch: Option<String>,
    pub status: Option<Status>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Requested,
    InProgress,
    Completed,
    Queued,
    Pending,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckRunConclusion {
    ActionRequired,
    Cancelled,
    Failure,
    Neutral,
    Skipped,
    /// Only github can set stale
    Stale,
    TimedOut,
    Success,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateCheckRun {
    pub name: String,
    pub head_sha: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<CheckRunOutput>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct PatchCheckRun {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<CheckRunConclusion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<CheckRunOutput>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct CheckRunOutput {
    pub title: String,
    pub summary: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRun {
    pub id: u64,
    pub name: String,
    pub head_sha: String,
    pub status: Status,
    pub conclusion: Option<CheckRunConclusion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub owner: User,
    pub default_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleInstallation {
    pub id: InstallationId,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationAccessToken {
    pub expires_at: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installation {
    pub id: InstallationId,
}
