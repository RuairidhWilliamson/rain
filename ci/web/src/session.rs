use axum::{
    extract::{Request, State},
    http::header::SET_COOKIE,
    middleware::Next,
    response::IntoResponse,
};
use axum_extra::{TypedHeader, headers};
use rain_ci_common::db::Db;

const SESSION_COOKIE_NAME: &str = "SESSION";

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
            session_id = new_session_id;
            changed = true;
        }
    } else {
        session_id = crate::db::create_session(&db).await?;
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
