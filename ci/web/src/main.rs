mod auth;
mod db;
mod github;
mod pages;
mod session;

use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::Result;
use axum::{
    Router,
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use log::info;
use oauth2::ClientSecret;
use secrecy::SecretString;

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
    db_password: Option<SecretString>,
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
    })
    .await?;
    let addr = config.addr;
    let github_config: GithubOauthConfig =
        serde_json::from_slice(&tokio::fs::read(&config.github_oauth_file).await?)?;
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
        .route("/", get(pages::home))
        .nest("/auth", auth::router())
        .route("/run", get(pages::runs))
        .route("/run/{id}", get(pages::run))
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
