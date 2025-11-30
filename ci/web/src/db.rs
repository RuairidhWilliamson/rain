use std::{path::PathBuf, str::FromStr as _};

use anyhow::{Context as _, Result, anyhow};
use chrono::{Days, NaiveDateTime, TimeDelta, Utc};
use oauth2::CsrfToken;
use rain_ci_common::{RepoHost, Repository, RepositoryId, Run, RunId, RunStatus};
use secrecy::{ExposeSecret as _, SecretString};

use crate::session::SessionId;

pub struct DbConfig {
    pub host: String,
    pub name: String,
    pub user: String,
    pub password: Option<SecretString>,
    pub password_file: Option<PathBuf>,
}

async fn load_password(config: &DbConfig) -> Result<SecretString> {
    if let Some(password) = &config.password {
        return Ok(password.clone());
    }
    if let Some(password_file) = &config.password_file {
        return Ok(tokio::fs::read_to_string(password_file)
            .await
            .context("cannot read DB_PASSWORD_FILE")?
            .into());
    }
    Err(anyhow!("set DB_PASSWORD or DB_PASSWORD_FILE"))
}

#[derive(Clone)]
pub struct Db {
    pool: sqlx::PgPool,
}

impl Db {
    pub async fn new(config: DbConfig) -> Result<Self> {
        let db_password = load_password(&config).await?;
        let pool = sqlx::PgPool::connect_with(
            sqlx::postgres::PgConnectOptions::new()
                .host(&config.host)
                .username(&config.user)
                .password(db_password.expose_secret())
                .database(&config.name),
        )
        .await?;
        Ok(Self { pool })
    }

    pub async fn create_session(&self) -> Result<SessionId> {
        let session_id = SessionId(uuid::Uuid::new_v4());
        let expires_at = (Utc::now() + Days::new(1)).naive_utc();
        sqlx::query!(
            "INSERT INTO sessions (id, expires_at) VALUES ($1, $2)",
            session_id.0,
            expires_at,
        )
        .execute(&self.pool)
        .await?;
        Ok(session_id)
    }

    pub async fn load_or_create_session(&self, id: &SessionId) -> Result<Option<SessionId>> {
        let mut tx = self.pool.begin().await?;
        if sqlx::query!(
            "SELECT id FROM sessions WHERE id=$1 AND expires_at > CURRENT_TIMESTAMP",
            id.0,
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_some()
        {
            return Ok(None);
        }
        let session_id = SessionId(uuid::Uuid::new_v4());
        let expires_at = (Utc::now() + Days::new(1)).naive_utc();
        sqlx::query!(
            "INSERT INTO sessions (id, expires_at) VALUES ($1, $2)",
            session_id.0,
            expires_at,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(session_id))
    }

    pub async fn set_session_csrf(&self, id: &SessionId, csrf: CsrfToken) -> Result<()> {
        sqlx::query!(
            "UPDATE sessions SET csrf=$2 WHERE id=$1",
            id.0,
            csrf.secret(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn check_session_csrf(&self, id: &SessionId, csrf: CsrfToken) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!("SELECT csrf FROM sessions WHERE id=$1", id.0)
            .fetch_one(&mut *tx)
            .await?;
        let expected: Option<String> = row.csrf;
        let expected = expected.ok_or_else(|| anyhow::format_err!("no csrf"))?;
        if !constant_time_eq::constant_time_eq(expected.as_bytes(), csrf.secret().as_bytes()) {
            return Err(anyhow::format_err!("session csrf does not match"));
        }
        sqlx::query!("UPDATE sessions SET csrf=NULL WHERE id=$1", id.0)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn auth_user_session(&self, id: &SessionId, user: super::User) -> Result<()> {
        sqlx::query!("INSERT INTO users (id, login, name, avatar_url) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING", user.0.id, user.0.login, user.0.name, user.0.avatar_url)
            .execute(&self.pool)
            .await?;
        sqlx::query!(
            "UPDATE sessions SET user_id=$1 WHERE id=$2",
            user.0.id,
            id.0
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user(&self, id: &SessionId) -> Result<Option<super::User>> {
        if let Some(user_row) = sqlx::query_as!(crate::github::UserDetails, "SELECT users.id, login, name, avatar_url FROM users INNER JOIN sessions ON users.id=sessions.user_id WHERE sessions.id=$1", id.0)
            .fetch_optional(&self.pool).await? {
            Ok(Some(super::User(user_row)))
        } else {
            Ok(None)
        }
    }

    pub async fn get_run(&self, id: &RunId) -> Result<Run> {
        let row = sqlx::query_file_as!(QueryRun, "queries/get_run.sql", id.0)
            .fetch_one(&self.pool)
            .await?;
        Ok(Run {
            commit: row.commit,
            created_at: row.created_at.and_utc(),
            dequeued_at: row.dequeued_at.map(|dt| dt.and_utc()),
            finished: row
                .finished_at
                .map(|finished_at| {
                    Result::<_>::Ok(rain_ci_common::FinishedRun {
                        finished_at: finished_at.and_utc(),
                        status: RunStatus::from_str(&row.status.context("status missing")?)
                            .context("unknown status")?,
                        execution_time: TimeDelta::milliseconds(
                            row.execution_time_millis
                                .context("execution_time_millis missing")?,
                        ),
                        output: row.output.context("output missing")?,
                    })
                })
                .transpose()?,
            repository: Repository {
                host: RepoHost::from_str(&row.host).context("unknown repo host")?,
                owner: row.owner,
                name: row.name,
            },
        })
    }

    pub async fn list_runs(&self) -> Result<Vec<(RunId, Run)>> {
        let rows = sqlx::query_file_as!(QueryRun, "queries/list_runs.sql")
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok((
                    RunId(row.id),
                    Run {
                        commit: row.commit,
                        created_at: row.created_at.and_utc(),
                        dequeued_at: row.dequeued_at.map(|dt| dt.and_utc()),
                        finished: row
                            .finished_at
                            .map(|finished_at| {
                                Result::<_>::Ok(rain_ci_common::FinishedRun {
                                    finished_at: finished_at.and_utc(),
                                    status: RunStatus::from_str(
                                        &row.status.context("status missing")?,
                                    )
                                    .context("unknown run status")?,
                                    execution_time: TimeDelta::milliseconds(
                                        row.execution_time_millis
                                            .context("execution_time_millis missing")?,
                                    ),
                                    output: row.output.context("output missing")?,
                                })
                            })
                            .transpose()?,
                        repository: Repository {
                            host: RepoHost::from_str(&row.host).context("unknown repo host")?,
                            owner: row.owner,
                            name: row.name,
                        },
                    },
                ))
            })
            .collect::<Result<_>>()
    }

    pub async fn list_repos(&self) -> Result<Vec<(RepositoryId, Repository)>> {
        let rows = sqlx::query_file_as!(QueryRepo, "queries/list_repos.sql")
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok((
                    RepositoryId(row.id),
                    Repository {
                        host: RepoHost::from_str(&row.host).context("unknown repo host")?,
                        owner: row.owner,
                        name: row.name,
                    },
                ))
            })
            .collect::<Result<_>>()
    }

    pub async fn get_repo(&self, id: &RepositoryId) -> Result<Repository> {
        let row = sqlx::query_file_as!(QueryRepo, "queries/get_repo.sql", id.0)
            .fetch_one(&self.pool)
            .await?;

        Ok(Repository {
            host: RepoHost::from_str(&row.host).context("unknown repo host")?,
            owner: row.owner,
            name: row.name,
        })
    }

    pub async fn create_run(&self, repo_id: &RepositoryId, commit: &str) -> Result<RunId> {
        // Check repo exists
        self.get_repo(repo_id).await?;
        let row = sqlx::query!(
            "INSERT INTO runs (created_at, repo, commit) VALUES ($1, $2, $3) RETURNING id",
            Utc::now().naive_utc(),
            repo_id.0,
            commit,
        )
        .fetch_one(&self.pool)
        .await?;
        let run_id = RunId(row.id);
        Ok(run_id)
    }
}

struct QueryRun {
    id: i64,
    host: String,
    owner: String,
    name: String,
    commit: String,
    created_at: NaiveDateTime,
    dequeued_at: Option<NaiveDateTime>,
    status: Option<String>,
    finished_at: Option<NaiveDateTime>,
    execution_time_millis: Option<i64>,
    output: Option<String>,
}

struct QueryRepo {
    id: i64,
    host: String,
    owner: String,
    name: String,
}
