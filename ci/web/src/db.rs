use std::path::PathBuf;

use anyhow::Result;
use oauth2::CsrfToken;
use tokio_postgres::NoTls;

use crate::session::SessionId;

#[derive(Clone)]
pub struct Db {
    pool: deadpool_postgres::Pool,
}

impl Db {
    pub fn new(host: String, name: String, user: String, password_file: PathBuf) -> Result<Self> {
        let mut config = deadpool_postgres::Config::new();
        config.dbname = Some(name);
        config.host = Some(host);
        config.user = Some(user);
        config.password = Some(std::fs::read_to_string(password_file)?.trim().into());
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
            .execute("INSERT INTO sessions (id) VALUES ($1)", &[&session_id])
            .await?;
        Ok(session_id)
    }

    pub async fn load_or_create_session(&self, id: &SessionId) -> Result<Option<SessionId>> {
        let mut conn = self.pool.get().await?;
        let tx = conn.transaction().await?;
        if tx
            .query_opt("SELECT id FROM sessions WHERE id=$1", &[id])
            .await?
            .is_some()
        {
            return Ok(None);
        }
        let session_id = SessionId(uuid::Uuid::new_v4());
        tx.execute("INSERT INTO sessions (id) VALUES ($1)", &[&session_id])
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

    pub async fn get_runs(&self) -> Result<Vec<(rain_ci_common::RunId, rain_ci_common::Run)>> {
        let conn = self.pool.get().await?;
        let rows = conn
            .query("SELECT id, source, created_at FROM runs LIMIT 100", &[])
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.get("id"),
                    rain_ci_common::Run {
                        source: r.get("source"),
                        created_at: r.get("created_at"),
                    },
                )
            })
            .collect())
    }
}
