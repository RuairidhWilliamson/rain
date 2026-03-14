use axum::{
    Extension, Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    routing::get,
};
use oauth2::{AuthorizationCode, CsrfToken, TokenResponse as _};
use rain_ci_common::db::Db;

use crate::{AppError, AppState, User, github, session};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/default", get(default_auth))
        .route("/github", get(github_auth))
        .route("/authorized", get(authorized))
}

async fn default_auth() -> impl IntoResponse {
    Redirect::to("/auth/github")
}

async fn github_auth(
    State(client): State<github::Client>,
    State(db): State<Db>,
    Extension(session): Extension<session::Session>,
) -> Result<impl IntoResponse, AppError> {
    let (auth_url, csrf_token) = client.authorize_url();
    crate::db::set_session_csrf(&db, &session.id, csrf_token).await?;
    Ok(Redirect::to(auth_url.as_ref()))
}

#[derive(Debug, serde::Deserialize)]
struct AuthRequest {
    code: AuthorizationCode,
    state: CsrfToken,
}

async fn authorized(
    Query(query): Query<AuthRequest>,
    State(client): State<github::Client>,
    State(db): State<Db>,
    Extension(session): Extension<session::Session>,
) -> Result<impl IntoResponse, AppError> {
    crate::db::check_session_csrf(&db, &session.id, query.state)
        .await
        .map_err(|err| anyhow::format_err!("csrf check failed: {err:#}"))?;
    let token = client.exchange_code(query.code).await?;
    let user = client.get_user_details(token.access_token()).await?;
    crate::db::auth_user_session(&db, &session.id, User(user))
        .await
        .map_err(|err| anyhow::format_err!("auth user session: {err:#}"))?;
    Ok(Redirect::to("/"))
}
