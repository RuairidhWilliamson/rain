use anyhow::Result;
use chrono::{Days, Utc};
use oauth2::CsrfToken;
use rain_ci_common::db::{Db, run::RunId};

use crate::session::SessionId;

const SESSION_EXPIRY: Days = Days::new(7);

pub async fn create_session(db: &Db) -> Result<SessionId> {
    let session_id = SessionId(uuid::Uuid::new_v4());
    let created_at = Utc::now().naive_utc();
    let expires_at = created_at + SESSION_EXPIRY;
    sqlx::query!(
        "INSERT INTO sessions (id, expires_at, created_at) VALUES ($1, $2, $3)",
        session_id.0,
        expires_at,
        created_at,
    )
    .execute(&db.pool)
    .await?;
    Ok(session_id)
}

pub async fn load_or_create_session(db: &Db, id: &SessionId) -> Result<Option<SessionId>> {
    let mut tx = db.pool.begin().await?;
    let creation_deadline = Utc::now().naive_utc() - SESSION_EXPIRY;
    if sqlx::query!(
            "SELECT id FROM sessions WHERE id=$1 AND expires_at > CURRENT_TIMESTAMP AND created_at > $2",
            id.0,
            creation_deadline,
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_some()
        {
            return Ok(None);
        }
    let session_id = SessionId(uuid::Uuid::new_v4());
    let created_at = Utc::now().naive_utc();
    let expires_at = created_at + SESSION_EXPIRY;
    sqlx::query!(
        "INSERT INTO sessions (id, expires_at, created_at) VALUES ($1, $2, $3)",
        session_id.0,
        expires_at,
        created_at,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(session_id))
}

pub async fn set_session_csrf(db: &Db, id: &SessionId, csrf: CsrfToken) -> Result<()> {
    sqlx::query!(
        "UPDATE sessions SET csrf=$2 WHERE id=$1",
        id.0,
        csrf.secret(),
    )
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn check_session_csrf(db: &Db, id: &SessionId, csrf: CsrfToken) -> Result<()> {
    let mut tx = db.pool.begin().await?;
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

pub async fn auth_user_session(db: &Db, id: &SessionId, user: super::User) -> Result<()> {
    sqlx::query!("INSERT INTO users (id, login, name, avatar_url) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING", user.0.id, user.0.login, user.0.name, user.0.avatar_url)
            .execute(&db.pool)
            .await?;
    sqlx::query!(
        "UPDATE sessions SET user_id=$1 WHERE id=$2",
        user.0.id,
        id.0
    )
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn get_user(db: &Db, id: SessionId) -> Result<Option<super::User>> {
    if let Some(user_row) = sqlx::query_as!(crate::github::UserDetails, "SELECT users.id, login, name, avatar_url FROM users INNER JOIN sessions ON users.id=sessions.user_id WHERE sessions.id=$1", id.0)
            .fetch_optional(&db.pool).await? {
            Ok(Some(super::User(user_row)))
        } else {
            Ok(None)
        }
}

pub async fn request_run(db: &Db, run: RunId) -> Result<()> {
    sqlx::query!("SELECT pg_notify('request_run', $1)", run.0.to_string())
        .execute(&db.pool)
        .await?;
    Ok(())
}
