mod auth;
mod db;
mod github;
mod pages;
mod session;

use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context as _, Result};
use axum::{
    Form, Router,
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts, Path, State},
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use jsonwebtoken::EncodingKey;
use log::info;
use oauth2::ClientSecret;
use rain_ci_common::{
    RepoHost, RepositoryId,
    github::{Client as _, InstallationClient as _},
};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;

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
    github_app_id: rain_ci_common::github::model::AppId,
    github_app_key_file: PathBuf,
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
    let key_raw = secrecy::SecretSlice::from(
        tokio::fs::read(&config.github_app_key_file)
            .await
            .context("read github app key")?,
    );
    let key =
        EncodingKey::from_rsa_pem(key_raw.expose_secret()).context("decode github app key")?;

    let github_client = Arc::new(rain_ci_common::github::implementation::AppClient::new(
        rain_ci_common::github::implementation::AppAuth {
            app_id: config.github_app_id,
            key,
        },
    ));
    let addr = config.addr;
    let github_oauth_config: GithubOauthConfig =
        serde_json::from_slice(&tokio::fs::read(&config.github_oauth_file).await?)?;
    let state = AppState {
        github_oauth_client: github::Client::new(
            github_oauth_config.github_client_id,
            github_oauth_config.github_client_secret,
            &config.base_url,
        )?,
        github_client,
        db,
        config: Arc::new(config),
    };
    let app = Router::new()
        .route("/", get(pages::home))
        .nest("/auth", auth::router())
        .route("/repos", get(pages::repos))
        .route("/repo/{id}", get(pages::repo))
        .route("/repo/{id}/run", post(repo_create_run))
        .route("/run", get(pages::runs))
        .route("/run/{id}", get(pages::run))
        .route("/assets/script.js", get(script_asset))
        .route("/assets/style.css", get(style_asset))
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

#[derive(Deserialize)]
struct RepoCreateRun {
    commit: String,
}

async fn repo_create_run(
    _auth: AdminUser,
    Path(repo_id): Path<RepositoryId>,
    State(db): State<db::Db>,
    State(github_app): State<Arc<rain_ci_common::github::implementation::AppClient>>,
    Form(data): Form<RepoCreateRun>,
) -> Result<impl IntoResponse, AppError> {
    let installations = github_app.app_installations().await?;
    // FIXME: Using the first installation is stupid
    let installation = installations.first().context("first installation")?;
    let installation_client = github_app.auth_installation(installation.id).await?;
    let db_repo = db.get_repo(&repo_id).await?;
    assert_eq!(db_repo.host, RepoHost::Github);
    let commit = installation_client
        .get_commit(&db_repo.owner, &db_repo.name, &data.commit)
        .await?;
    let run_id = db.create_run(&repo_id, &commit.sha).await?;
    Ok(Redirect::to(&format!("/run/{run_id}")))
}

async fn script_asset() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!("../assets/script.js"),
    )
}

async fn style_asset() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../assets/style.css"),
    )
}

#[derive(FromRef, Clone)]
struct AppState {
    github_oauth_client: github::Client,
    github_client: Arc<rain_ci_common::github::implementation::AppClient>,
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
