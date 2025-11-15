use std::path::PathBuf;

use anyhow::{Context as _, Result};
use chrono::{Days, NaiveDateTime, TimeDelta, Utc};
use oauth2::CsrfToken;
use rain_ci_common::{RunId, RunSource, RunStatus};

use crate::session::SessionId;

pub struct DbConfig {
    pub host: String,
    pub name: String,
    pub user: String,
    pub password: Option<String>,
    pub password_file: Option<PathBuf>,
}

#[derive(Clone)]
pub struct Db {
    pool: sqlx::PgPool,
}

impl Db {
    pub async fn new(cfg: DbConfig) -> Result<Self> {
        let db_password = cfg
            .password
            .or_else(|| std::fs::read_to_string(cfg.password_file.as_ref()?).ok())
            .context("set DB_PASSWORD or DB_PASSWORD_FILE")?;
        let pool = sqlx::PgPool::connect_with(
            sqlx::postgres::PgConnectOptions::new()
                .host(&cfg.host)
                .username(&cfg.user)
                .password(&db_password)
                .database(&cfg.name),
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

    pub async fn get_run(&self, id: &RunId) -> Result<rain_ci_common::Run> {
        struct QueryRun {
            repo_owner: String,
            repo_name: String,
            source: RunSource,
            commit: String,
            created_at: NaiveDateTime,
            status: Option<RunStatus>,
            dequeued_at: Option<NaiveDateTime>,
            finished_at: Option<NaiveDateTime>,
            execution_time_millis: Option<i64>,
            output: Option<String>,
        }
        let row = sqlx::query_file_as!(QueryRun, "queries/get_run.sql", id.0)
            .fetch_one(&self.pool)
            .await?;
        Ok(rain_ci_common::Run {
            source: row.source,
            commit: row.commit,
            created_at: row.created_at,
            dequeued_at: row.dequeued_at,
            finished: row
                .finished_at
                .map(|finished_at| rain_ci_common::FinishedRun {
                    finished_at,
                    status: row.status.unwrap(),
                    execution_time: TimeDelta::milliseconds(row.execution_time_millis.unwrap()),
                    output: row.output.unwrap(),
                }),
            repository: rain_ci_common::Repository {
                owner: row.repo_owner,
                name: row.repo_name,
            },
        })
    }

    pub async fn list_runs(&self) -> Result<Vec<(rain_ci_common::RunId, rain_ci_common::Run)>> {
        struct QueryRun {
            id: i64,
            source: RunSource,
            repo_owner: String,
            repo_name: String,
            commit: String,
            created_at: NaiveDateTime,
            dequeued_at: Option<NaiveDateTime>,
            status: Option<RunStatus>,
            finished_at: Option<NaiveDateTime>,
            execution_time_millis: Option<i64>,
            output: Option<String>,
        }
        let rows = sqlx::query_file_as!(QueryRun, "queries/list_runs.sql")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    RunId(row.id),
                    rain_ci_common::Run {
                        source: row.source,
                        commit: row.commit,
                        created_at: row.created_at,
                        dequeued_at: row.dequeued_at,
                        finished: row
                            .finished_at
                            .map(|finished_at| rain_ci_common::FinishedRun {
                                finished_at,
                                status: row.status.unwrap(),
                                execution_time: TimeDelta::milliseconds(
                                    row.execution_time_millis.unwrap(),
                                ),
                                output: row.output.unwrap(),
                            }),
                        repository: rain_ci_common::Repository {
                            owner: row.repo_owner,
                            name: row.repo_name,
                        },
                    },
                )
            })
            .collect())
    }
}
