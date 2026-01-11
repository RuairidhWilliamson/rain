use std::{path::PathBuf, str::FromStr as _};

use anyhow::{Context as _, Result, anyhow};
use chrono::{Days, NaiveDateTime, TimeDelta, Utc};
use oauth2::CsrfToken;
use rain_ci_common::{RepoHost, Repository, RepositoryId, Run, RunId, RunStatus};
use secrecy::{ExposeSecret as _, SecretString};

use crate::{
    pagination::{Paginated, Pagination},
    session::SessionId,
};

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
    per_page: u64,
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
        Ok(Self { pool, per_page: 25 })
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
        row.convert()
    }

    pub async fn list_runs(&self, page: &Pagination) -> Result<Paginated<(RunId, Run)>> {
        let mut tx = self.pool.begin().await?;
        let per_page = i64::try_from(self.per_page)?;
        let rows = sqlx::query_file_as!(
            QueryRun,
            "queries/list_runs.sql",
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count_row =
            sqlx::query!("SELECT COUNT(*) FROM runs INNER JOIN repos ON runs.repo=repos.id")
                .fetch_one(&mut *tx)
                .await?;

        tx.rollback().await?;

        let elements: Vec<(RunId, Run)> = rows
            .into_iter()
            .map(|row| Ok((RunId(row.id), row.convert()?)))
            .collect::<Result<_>>()?;
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(elements, full_count, self.per_page, page))
    }

    pub async fn list_repos(
        &self,
        page: &Pagination,
    ) -> Result<Paginated<(RepositoryId, Repository)>> {
        let mut tx = self.pool.begin().await?;
        let per_page = i64::try_from(self.per_page)?;
        let rows = sqlx::query_file_as!(
            QueryRepo,
            "queries/list_repos.sql",
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&self.pool)
        .await?;
        let count_row = sqlx::query!("SELECT COUNT(*) FROM repos")
            .fetch_one(&mut *tx)
            .await?;

        tx.rollback().await?;

        let elements = rows
            .into_iter()
            .map(|row| {
                Ok((
                    RepositoryId(row.id),
                    Repository {
                        id: rain_ci_common::RepositoryId(row.id),
                        host: RepoHost::from_str(&row.host).context("unknown repo host")?,
                        owner: row.owner,
                        name: row.name,
                    },
                ))
            })
            .collect::<Result<_>>()?;
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();

        Ok(Paginated::new(elements, full_count, self.per_page, page))
    }

    pub async fn get_repo(&self, id: &RepositoryId) -> Result<Repository> {
        let row = sqlx::query_file_as!(QueryRepo, "queries/get_repo.sql", id.0)
            .fetch_one(&self.pool)
            .await?;

        Ok(Repository {
            id: *id,
            host: RepoHost::from_str(&row.host).context("unknown repo host")?,
            owner: row.owner,
            name: row.name,
        })
    }

    pub async fn create_run(
        &self,
        repo_id: &RepositoryId,
        commit: &str,
        target: &str,
    ) -> Result<RunId> {
        // Check repo exists
        self.get_repo(repo_id).await?;
        let row = sqlx::query!(
            "INSERT INTO runs (created_at, repo, commit, target) VALUES ($1, $2, $3, $4) RETURNING id",
            Utc::now().naive_utc(),
            repo_id.0,
            commit,
            target,
        )
        .fetch_one(&self.pool)
        .await?;
        let run_id = RunId(row.id);
        sqlx::query!("SELECT pg_notify('request_run', $1)", run_id.0.to_string())
            .execute(&self.pool)
            .await?;
        Ok(run_id)
    }

    pub async fn list_runs_in_repo(
        &self,
        repo_id: &RepositoryId,
        page: &Pagination,
    ) -> Result<Paginated<(RunId, Run)>> {
        // Check repo exists
        self.get_repo(repo_id).await?;
        let mut tx = self.pool.begin().await?;
        let per_page = i64::try_from(self.per_page)?;
        let rows = sqlx::query_file_as!(
            QueryRun,
            "queries/list_runs_in_repo.sql",
            repo_id.0,
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&self.pool)
        .await?;

        let count_row = sqlx::query!(
            "SELECT COUNT(*) FROM runs INNER JOIN repos ON runs.repo=repos.id WHERE repos.id=$1",
            repo_id.0
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.rollback().await?;

        let elements: Vec<(RunId, Run)> = rows
            .into_iter()
            .map(|row| Ok((RunId(row.id), row.convert()?)))
            .collect::<Result<_>>()?;
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(elements, full_count, self.per_page, page))
    }
}

struct QueryRun {
    id: i64,
    repo_id: i64,
    host: String,
    owner: String,
    name: String,
    commit: String,
    target: String,
    created_at: NaiveDateTime,
    dequeued_at: Option<NaiveDateTime>,
    rain_version: Option<String>,
    status: Option<String>,
    finished_at: Option<NaiveDateTime>,
    execution_time_millis: Option<i64>,
    output: Option<String>,
}

impl QueryRun {
    fn convert(self) -> Result<Run> {
        let row = self;
        Ok(Run {
            commit: row.commit,
            created_at: row.created_at.and_utc(),
            dequeued_at: row.dequeued_at.map(|dt| dt.and_utc()),
            rain_version: row.rain_version,
            target: row.target,
            finished: row
                .finished_at
                .map(|finished_at| {
                    Result::<_>::Ok(rain_ci_common::FinishedRun {
                        finished_at: finished_at.and_utc(),
                        status: RunStatus::from_str(&row.status.context("status missing")?)
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
                id: rain_ci_common::RepositoryId(row.repo_id),
                host: RepoHost::from_str(&row.host).context("unknown repo host")?,
                owner: row.owner,
                name: row.name,
            },
        })
    }
}

struct QueryRepo {
    id: i64,
    host: String,
    owner: String,
    name: String,
}
