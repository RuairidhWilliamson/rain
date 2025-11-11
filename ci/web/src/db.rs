use std::path::PathBuf;

use anyhow::{Context as _, Result};
use chrono::{Days, TimeDelta, Utc};
use oauth2::CsrfToken;
use rain_ci_common::RunId;
use tokio_postgres::NoTls;

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
    pool: deadpool_postgres::Pool,
}

impl Db {
    pub fn new(cfg: DbConfig) -> Result<Self> {
        let db_password = cfg
            .password
            .or_else(|| std::fs::read_to_string(cfg.password_file.as_ref()?).ok())
            .context("set DB_PASSWORD or DB_PASSWORD_FILE")?;
        let mut config = deadpool_postgres::Config::new();
        config.dbname = Some(cfg.name);
        config.host = Some(cfg.host);
        config.user = Some(cfg.user);
        config.password = Some(db_password);
        config.manager = Some(deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        });
        let pool = config.create_pool(Some(deadpool_postgres::Runtime::Tokio1), NoTls)?;
        Ok(Self { pool })
    }

    pub async fn create_session(&self) -> Result<SessionId> {
        let session_id = SessionId(uuid::Uuid::new_v4());
        self.pool
            .get()
            .await?
            .execute(
                "INSERT INTO sessions (id, expires_at) VALUES ($1, $2)",
                &[&session_id, &(Utc::now() + Days::new(1)).naive_utc()],
            )
            .await?;
        Ok(session_id)
    }

    pub async fn load_or_create_session(&self, id: &SessionId) -> Result<Option<SessionId>> {
        let mut conn = self.pool.get().await?;
        let tx = conn.transaction().await?;
        if tx
            .query_opt(
                "SELECT id FROM sessions WHERE id=$1 AND expires_at > CURRENT_TIMESTAMP",
                &[id],
            )
            .await?
            .is_some()
        {
            return Ok(None);
        }
        let session_id = SessionId(uuid::Uuid::new_v4());
        tx.execute(
            "INSERT INTO sessions (id, expires_at) VALUES ($1, $2)",
            &[&session_id, &(Utc::now() + Days::new(1)).naive_utc()],
        )
        .await?;
        tx.commit().await?;
        Ok(Some(session_id))
    }

    pub async fn set_session_csrf(&self, id: &SessionId, csrf: CsrfToken) -> Result<()> {
        let conn = self.pool.get().await?;
        conn.execute(
            "UPDATE sessions SET csrf=$2 WHERE id=$1",
            &[id, csrf.secret()],
        )
        .await?;
        Ok(())
    }

    pub async fn check_session_csrf(&self, id: &SessionId, csrf: CsrfToken) -> Result<()> {
        let mut conn = self.pool.get().await?;
        let tx = conn.transaction().await?;
        let row = tx
            .query_one("SELECT csrf FROM sessions WHERE id=$1", &[id])
            .await?;
        let expected: Option<String> = row.get("csrf");
        let expected = expected.ok_or_else(|| anyhow::format_err!("no csrf"))?;
        if !constant_time_eq::constant_time_eq(expected.as_bytes(), csrf.secret().as_bytes()) {
            return Err(anyhow::format_err!("session csrf does not match"));
        }
        tx.execute("UPDATE sessions SET csrf=NULL WHERE id=$1", &[id])
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn auth_user_session(&self, id: &SessionId, user: super::User) -> Result<()> {
        let conn = self.pool.get().await?;
        conn.execute(
            "INSERT INTO users (id, login, name, avatar_url) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            &[&user.0.id, &user.0.login, &user.0.name, &user.0.avatar_url],
        )
        .await?;
        conn.execute(
            "UPDATE sessions SET user_id=$1 WHERE id=$2",
            &[&user.0.id, id],
        )
        .await?;
        Ok(())
    }

    pub async fn get_user(&self, id: &SessionId) -> Result<Option<super::User>> {
        let conn = self.pool.get().await?;
        if let Some(user_row) = conn.query_opt("SELECT users.id, login, name, avatar_url FROM users INNER JOIN sessions ON users.id=sessions.user_id WHERE sessions.id=$1", &[id]).await? {
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
        let conn = self.pool.get().await?;
        let row = conn
            .query_one(
                "SELECT runs.id, source, commit, created_at, dequeued_at, repo_owner, repo_name, finished_at, status, execution_time_millis, output FROM runs LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run WHERE id=$1",
                &[id],
            )
            .await?;
        Ok(rain_ci_common::Run {
            source: row.get("source"),
            commit: row.get("commit"),
            created_at: row.get("created_at"),
            dequeued_at: row.get("dequeued_at"),
            finished: row.get::<_, Option<_>>("finished_at").map(|finished_at| {
                rain_ci_common::FinishedRun {
                    finished_at,
                    status: row.get("status"),
                    execution_time: TimeDelta::milliseconds(row.get("execution_time_millis")),
                    output: row.get("output"),
                }
            }),
            repository: rain_ci_common::Repository {
                owner: row.get("repo_owner"),
                name: row.get("repo_name"),
            },
        })
    }

    pub async fn get_runs(&self) -> Result<Vec<(rain_ci_common::RunId, rain_ci_common::Run)>> {
        let conn = self.pool.get().await?;
        let rows = conn
            .query(
                "SELECT runs.id, source, commit, created_at, dequeued_at, repo_owner, repo_name, finished_at, status, execution_time_millis, output FROM runs LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run ORDER BY id DESC LIMIT 100",
                &[],
            )
            .await?;
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
                        finished: row.get::<_, Option<_>>("finished_at").map(|finished_at| {
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
