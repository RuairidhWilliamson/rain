use std::path::PathBuf;

use anyhow::{Context as _, Result};
use chrono::{Days, TimeDelta, Utc};
use oauth2::CsrfToken;
use rain_ci_common::RunId;
use sqlx::Row;

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
        sqlx::query("INSERT INTO sessions (id, expires_at) VALUES ($1, $2)")
            .bind(session_id)
            .bind((Utc::now() + Days::new(1)).naive_utc())
            .execute(&self.pool)
            .await?;
        Ok(session_id)
    }

    pub async fn load_or_create_session(&self, id: &SessionId) -> Result<Option<SessionId>> {
        let mut tx = self.pool.begin().await?;
        if sqlx::query("SELECT id FROM sessions WHERE id=$1 AND expires_at > CURRENT_TIMESTAMP")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some()
        {
            return Ok(None);
        }
        let session_id = SessionId(uuid::Uuid::new_v4());
        sqlx::query("INSERT INTO sessions (id, expires_at) VALUES ($1, $2)")
            .bind(session_id)
            .bind((Utc::now() + Days::new(1)).naive_utc())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(session_id))
    }

    pub async fn set_session_csrf(&self, id: &SessionId, csrf: CsrfToken) -> Result<()> {
        sqlx::query("UPDATE sessions SET csrf=$2 WHERE id=$1")
            .bind(id)
            .bind(csrf.secret())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn check_session_csrf(&self, id: &SessionId, csrf: CsrfToken) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT csrf FROM sessions WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        let expected: Option<String> = row.get("csrf");
        let expected = expected.ok_or_else(|| anyhow::format_err!("no csrf"))?;
        if !constant_time_eq::constant_time_eq(expected.as_bytes(), csrf.secret().as_bytes()) {
            return Err(anyhow::format_err!("session csrf does not match"));
        }
        sqlx::query("UPDATE sessions SET csrf=NULL WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn auth_user_session(&self, id: &SessionId, user: super::User) -> Result<()> {
        sqlx::query("INSERT INTO users (id, login, name, avatar_url) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
            .bind(user.0.id)
            .bind(user.0.login)
            .bind(user.0.name)
            .bind(user.0.avatar_url)
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE sessions SET user_id=$1 WHERE id=$2")
            .bind(user.0.id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user(&self, id: &SessionId) -> Result<Option<super::User>> {
        if let Some(user_row) = sqlx::query("SELECT users.id, login, name, avatar_url FROM users INNER JOIN sessions ON users.id=sessions.user_id WHERE sessions.id=$1")
            .bind(id)
            .fetch_optional(&self.pool).await? {
            Ok(Some(super::User (crate::github::UserDetails {
                 id: user_row.get("id"),
                 login: user_row.get("login"),
                 name: user_row.get("name"),
                 avatar_url: user_row.get("avatar_url")
             })))
        } else {
            Ok(None)
        }
    }

    pub async fn get_run(&self, id: &RunId) -> Result<rain_ci_common::Run> {
        let row = sqlx::query("SELECT runs.id, source, commit, created_at, dequeued_at, repo_owner, repo_name, finished_at, status, execution_time_millis, output FROM runs LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run WHERE id=$1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(rain_ci_common::Run {
            source: row.get("source"),
            commit: row.get("commit"),
            created_at: row.get("created_at"),
            dequeued_at: row.get("dequeued_at"),
            finished: row
                .get::<Option<_>, &str>("finished_at")
                .map(|finished_at| rain_ci_common::FinishedRun {
                    finished_at,
                    status: row.get("status"),
                    execution_time: TimeDelta::milliseconds(row.get("execution_time_millis")),
                    output: row.get("output"),
                }),
            repository: rain_ci_common::Repository {
                owner: row.get("repo_owner"),
                name: row.get("repo_name"),
            },
        })
    }

    pub async fn get_runs(&self) -> Result<Vec<(rain_ci_common::RunId, rain_ci_common::Run)>> {
        let rows = sqlx::query("SELECT runs.id, source, commit, created_at, dequeued_at, repo_owner, repo_name, finished_at, status, execution_time_millis, output FROM runs LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run ORDER BY id DESC LIMIT 100")
            .fetch_all(&self.pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get("id"),
                    rain_ci_common::Run {
                        source: row.get("source"),
                        commit: row.get("commit"),
                        created_at: row.get("created_at"),
                        dequeued_at: row.get("dequeued_at"),
                        finished: row.get::<Option<_>, _>("finished_at").map(|finished_at| {
                            rain_ci_common::FinishedRun {
                                finished_at,
                                status: row.get("status"),
                                execution_time: TimeDelta::milliseconds(
                                    row.get("execution_time_millis"),
                                ),
                                output: row.get("output"),
                            }
                        }),
                        repository: rain_ci_common::Repository {
                            owner: row.get("repo_owner"),
                            name: row.get("repo_name"),
                        },
                    },
                )
            })
            .collect())
    }
}
