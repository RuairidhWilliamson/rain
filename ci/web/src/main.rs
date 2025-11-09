mod db;
mod github;
mod session;

use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::Result;
use askama::Template;
use axum::{
    Extension, Router,
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts, Query, State},
    http::{StatusCode, request::Parts},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use log::info;
use oauth2::{AuthorizationCode, ClientSecret, CsrfToken, TokenResponse as _};

#[derive(Debug, serde::Deserialize)]
struct Config {
    base_url: String,
    addr: SocketAddr,
    github_oauth_file: PathBuf,
    allowed_github_user_id: i64,
    allowed_github_login: String,
    db_host: String,
    db_name: String,
    db_user: String,
    db_password: Option<String>,
    db_password_file: Option<PathBuf>,
}

#[derive(Debug, serde::Deserialize)]
struct GithubOauthConfig {
    github_client_id: String,
    github_client_secret: ClientSecret,
}

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();
    env_logger::init();
    if let Err(err) = dotenv_result {
        log::warn!(".env could not be loaded: {err:#}");
    }
    let config = envy::from_env::<Config>()?;
    let version = env!("CARGO_PKG_VERSION");
    info!("version = {version}");
    let db = db::Db::new(db::DbConfig {
        host: config.db_host.clone(),
        name: config.db_name.clone(),
        user: config.db_user.clone(),
        password: config.db_password.clone(),
        password_file: config.db_password_file.clone(),
    })?;
    let addr = config.addr;
    let github_config: GithubOauthConfig =
        serde_json::from_slice(&std::fs::read(&config.github_oauth_file)?)?;
    let state = AppState {
        github_client: github::Client::new(
            github_config.github_client_id,
            github_config.github_client_secret,
            &config.base_url,
        )?,
        db,
        config: Arc::new(config),
    };
    let app = Router::new()
        .route("/", get(homepage))
        .route("/admin", get(adminpage))
        .route("/auth/default", get(default_auth))
        .route("/auth/github", get(github_auth))
        .route("/auth/authorized", get(authorized))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            session::session_middleware,
        ))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

#[derive(Template)]
#[template(
    ext = "html",
    source = "
        <h1>Hello, World!</h1>
        <a href='/auth/default'>Auth</a><span></span>
    "
)]
struct PublicHomepage;

#[derive(Template)]
#[template(
    ext = "html",
    source = "
        <h1>Hello, {{name}}!</h1>
        <div>
            <img src={{avatar_url}} height=16>
            <span>{{name}}</span>
        </div>
        <a href='/admin'>Admin</a>
    "
)]
struct Homepage<'a> {
    name: &'a str,
    avatar_url: &'a str,
}

async fn homepage(auth: Option<AuthUser>) -> Result<Html<String>, AppError> {
    if let Some(auth) = auth {
        let homepage = Homepage {
            name: &auth.user.0.name,
            avatar_url: &auth.user.0.avatar_url,
        };
        Ok(Html(homepage.render()?))
    } else {
        let homepage = PublicHomepage;
        Ok(Html(homepage.render()?))
    }
}

#[derive(Template)]
#[template(
    ext = "html",
    source = "
        <h1>Admin</h1>
        <div>
            <img src={{avatar_url}} height=16>
            <span>{{name}}</span>
        </div>
        <table>
        {% for (run_id, run) in runs %}
            <tr>
                <td>{{ run_id }}</td>
                <td>{{ run.source }}</td>
                <td>{{ run.repository.owner }}/{{ run.repository.name }}</td>
                <td>{{ run.created_at }}</td>
                <td>{{ run.state() }}</td>
            </tr>
        {% endfor %}
        </table>
    "
)]
struct AdminPage<'a> {
    name: &'a str,
    avatar_url: &'a str,
    runs: &'a [(rain_ci_common::RunId, rain_ci_common::Run)],
}

async fn adminpage(auth: AdminUser, State(db): State<db::Db>) -> Result<Html<String>, AppError> {
    let admin_page = AdminPage {
        name: &auth.user.0.name,
        avatar_url: &auth.user.0.avatar_url,
        runs: &db.get_runs().await?,
    };
    Ok(Html(admin_page.render()?))
}

async fn default_auth() -> impl IntoResponse {
    Redirect::to("/auth/github")
}

async fn github_auth(
    State(client): State<github::Client>,
    State(db): State<db::Db>,
    Extension(session): Extension<session::Session>,
) -> Result<impl IntoResponse, AppError> {
    let (auth_url, csrf_token) = client.authorize_url();
    db.set_session_csrf(&session.id, csrf_token).await?;
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
    State(db): State<db::Db>,
    Extension(session): Extension<session::Session>,
) -> Result<impl IntoResponse, AppError> {
    db.check_session_csrf(&session.id, query.state)
        .await
        .map_err(|err| anyhow::format_err!("csrf check failed: {err:#}"))?;
    let token = client.exchange_code(query.code).await?;
    let user = client.get_user_details(token.access_token()).await?;
    db.auth_user_session(&session.id, User(user))
        .await
        .map_err(|err| anyhow::format_err!("auth user session: {err:#}"))?;
    Ok(Redirect::to("/"))
}

#[derive(FromRef, Clone)]
struct AppState {
    github_client: github::Client,
    db: db::Db,
    config: Arc<Config>,
}

#[derive(Debug)]
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        log::error!("Application error: {:#}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/auth/default").into_response()
    }
}

#[derive(Clone)]
struct User(github::UserDetails);

impl User {
    fn is_admin(&self, config: &Config) -> bool {
        self.0.id == config.allowed_github_user_id && self.0.login == config.allowed_github_login
    }
}

struct AuthUser {
    user: User,
}

impl<S> FromRequestParts<S> for AuthUser
where
    db::Db: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = db::Db::from_ref(state);

        let Some(session): Option<&session::Session> = parts.extensions.get() else {
            unreachable!("get session extension");
        };
        let user = store
            .get_user(&session.id)
            .await
            .map_err(|err| {
                log::error!("get user: {err:#}");
                AuthRedirect
            })?
            .ok_or(AuthRedirect)?;

        Ok(Self { user })
    }
}

impl<S> OptionalFromRequestParts<S> for AuthUser
where
    db::Db: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <Self as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(res) => Ok(Some(res)),
            Err(AuthRedirect) => Ok(None),
        }
    }
}

struct AdminUser {
    user: User,
}

impl<S> FromRequestParts<S> for AdminUser
where
    db::Db: FromRef<S>,
    Arc<Config>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = db::Db::from_ref(state);
        let config = Arc::<Config>::from_ref(state);

        let Some(session): Option<&session::Session> = parts.extensions.get() else {
            unreachable!("get session extension");
        };
        let user = store
            .get_user(&session.id)
            .await
            .map_err(|err| {
                log::error!("get user: {err:#}");
                StatusCode::UNAUTHORIZED
            })?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        if !user.is_admin(&config) {
            return Err(StatusCode::UNAUTHORIZED);
        }

        Ok(Self { user })
    }
}

impl<S> OptionalFromRequestParts<S> for AdminUser
where
    db::Db: FromRef<S>,
    Arc<Config>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <Self as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(res) => Ok(Some(res)),
            Err(_) => Ok(None),
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    log::info!("signal received, starting graceful shutdown");
}
