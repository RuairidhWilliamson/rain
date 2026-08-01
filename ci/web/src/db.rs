use anyhow::Result;
use chrono::{Days, Utc};
use oauth2::CsrfToken;
use rain_ci_common::db::{Db, run::RunId};
use secrecy::SecretString;

use crate::{auth::UserDetails, session::SessionId};

const SESSION_EXPIRY: Days = Days::new(7);

pub async fn create_session(db: &Db) -> Result<SessionId> {
    let session_id = SessionId(uuid::Uuid::new_v4());
    let created_at = Utc::now().naive_utc();
    let expires_at = created_at + SESSION_EXPIRY;
    sqlx::query!(
        "INSERT INTO sessions (id, expires_at, created_at, active) VALUES ($1, $2, $3, true)",
        session_id.0,
        expires_at,
        created_at,
    )
    .execute(&db.pool)
    .await?;
    Ok(session_id)
}

pub async fn delete_session(db: &Db, id: &SessionId) -> Result<()> {
    sqlx::query!("UPDATE sessions SET active=false WHERE id=$1", id.0)
        .execute(&db.pool)
        .await?;
    Ok(())
}

pub async fn load_or_create_session(db: &Db, id: &SessionId) -> Result<Option<SessionId>> {
    let mut tx = db.pool.begin().await?;
    let creation_deadline = Utc::now().naive_utc() - SESSION_EXPIRY;
    if sqlx::query!(
            "SELECT id FROM sessions WHERE id=$1 AND expires_at > CURRENT_TIMESTAMP AND created_at > $2 AND active",
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
        "INSERT INTO sessions (id, expires_at, created_at, active) VALUES ($1, $2, $3, true)",
        session_id.0,
        expires_at,
        created_at,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(session_id))
}

pub async fn set_session_csrf(
    db: &Db,
    id: &SessionId,
    csrf: CsrfToken,
    next_url: Option<String>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE sessions SET csrf=$2, next_url=$3 WHERE id=$1",
        id.0,
        csrf.secret(),
        next_url,
    )
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn check_session_csrf(
    db: &Db,
    id: &SessionId,
    csrf: CsrfToken,
) -> Result<Option<String>> {
    let mut tx = db.pool.begin().await?;
    let row = sqlx::query!("SELECT csrf, next_url FROM sessions WHERE id=$1", id.0)
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
    Ok(row.next_url)
}

pub async fn auth_user_session(
    db: &Db,
    id: &SessionId,
    auth_provider: &str,
    user: UserDetails,
) -> Result<()> {
    let mut tx = db.pool.begin().await?;
    let user_id;
    if let Some(row) = sqlx::query!(
        "
        SELECT id FROM users WHERE provider=$1 AND email=$2
        ",
        auth_provider,
        &user.email,
    )
    .fetch_optional(&mut *tx)
    .await?
    {
        user_id = row.id;
    } else {
        let row = sqlx::query!(
            "
        INSERT INTO users (login, email, name, avatar_url, provider)
        VALUES ($1, $2, $3, $4, $5) RETURNING id
        ",
            user.login,
            user.email,
            user.name,
            user.avatar_url,
            auth_provider,
        )
        .fetch_one(&mut *tx)
        .await?;
        user_id = row.id;
    }
    sqlx::query!("UPDATE sessions SET user_id=$1 WHERE id=$2", user_id, id.0)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn get_user(db: &Db, id: SessionId) -> Result<Option<crate::user::User>> {
    if let Some(user_row) = sqlx::query_as!(
        crate::user::User,
        "
        SELECT users.id, email, login, name, avatar_url FROM users
        INNER JOIN sessions ON users.id=sessions.user_id
        WHERE sessions.id=$1
        ",
        id.0
    )
    .fetch_optional(&db.pool)
    .await?
    {
        Ok(Some(user_row))
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

pub async fn cancel_run(db: &Db, run: RunId) -> Result<()> {
    sqlx::query!("SELECT pg_notify('cancel_run', $1)", run.0.to_string())
        .execute(&db.pool)
        .await?;
    Ok(())
}

#[derive(Debug)]
pub struct AuthProvider {
    pub name: String,
    pub kind: AuthKind,
    pub oidc_discovery_url: Option<String>,
    pub client_id: String,
    pub client_secret: SecretString,
    pub certificate: Option<String>,
}

#[derive(Debug, Clone, sqlx::Type)]
pub enum AuthKind {
    Github,
    OpenIDConnect,
}

pub async fn get_auth_provider(db: &Db, name: &str) -> Result<AuthProvider> {
    let auth_provider = sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT name, kind as "kind: AuthKind", oidc_discovery_url, client_id, client_secret, certificate
        FROM auth_providers WHERE name=$1
        "#,
        name,
    )
    .fetch_one(&db.pool)
    .await?;
    Ok(auth_provider)
}
