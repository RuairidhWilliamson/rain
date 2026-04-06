use axum::{
    Extension,
    extract::{Request, State},
    http::header::SET_COOKIE,
    middleware::Next,
    response::{IntoResponse, Redirect},
};
use axum_extra::{TypedHeader, headers};
use rain_ci_common::db::Db;

const SESSION_COOKIE_NAME: &str = "__Host-Http-SESSION";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct SessionId(pub uuid::Uuid);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Clone)]
pub struct Session {
    pub id: SessionId,
}

pub async fn session_middleware(
    cookie: Option<TypedHeader<headers::Cookie>>,
    State(db): State<Db>,
    mut request: Request,
    next: Next,
) -> Result<impl IntoResponse, super::AppError> {
    let mut session_id: SessionId;
    let mut changed = false;
    if let Some(cookie) = cookie
        && let Some(session_cookie) = cookie.get(SESSION_COOKIE_NAME)
        && let Ok(inner_session_id) = session_cookie.parse::<uuid::Uuid>()
    {
        session_id = SessionId(inner_session_id);
        if let Some(new_session_id) = crate::db::load_or_create_session(&db, &session_id).await? {
            tracing::debug!("changed session {} -> {}", session_id, new_session_id);
            session_id = new_session_id;
            changed = true;
        } else {
            tracing::debug!("restored session {}", session_id);
        }
    } else {
        session_id = crate::db::create_session(&db).await?;
        tracing::debug!("created new session {}", session_id);
        changed = true;
    }

    request.extensions_mut().insert(Session { id: session_id });

    let mut response = next.run(request).await;

    if changed {
        response.headers_mut().insert(
            SET_COOKIE,
            format!("{SESSION_COOKIE_NAME}={session_id}; SameSite=Lax; HttpOnly; Secure; Path=/")
                .parse()?,
        );
    }

    Ok(response)
}

pub async fn signout(
    Extension(session): Extension<Session>,
    State(db): State<Db>,
) -> Result<impl IntoResponse, super::AppError> {
    crate::db::delete_session(&db, &session.id).await?;
    Ok(Redirect::to("/"))
}
