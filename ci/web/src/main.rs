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
use chrono::Utc;
use log::info;
use oauth2::ClientSecret;
use rain_ci_common::{
    db::{
        Db, DbConfig, Resource as _,
        repository::{Repository, RepositoryId},
        repository_host::{RepoHostKind, RepositoryHost},
        run::Run,
    },
    github::{
        Client as _, InstallationClient as _,
        implementation::{AppAuth, AppClient},
        model::AppId,
    },
};
use secrecy::ExposeSecret as _;
use serde::Deserialize;
use url::Url;

#[derive(Debug, serde::Deserialize)]
struct Config {
    base_url: String,
    addr: SocketAddr,
    github_oauth_file: PathBuf,
    allowed_github_user_id: i64,
    allowed_github_login: String,
    database_password_file: Option<PathBuf>,
    database_url: Url,
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
    let db = Db::new(
        DbConfig {
            url: config.database_url.clone(),
            password_file: config.database_password_file.clone(),
        },
        "rain-ci-web",
    )
    .await?;
    let addr = config.addr;
    let github_oauth_config: GithubOauthConfig =
        serde_json::from_slice(&tokio::fs::read(&config.github_oauth_file).await?)?;
    let state = AppState {
        github_oauth_client: github::Client::new(
            github_oauth_config.github_client_id,
            github_oauth_config.github_client_secret,
            &config.base_url,
        )?,
        db,
        config: Arc::new(config),
    };
    let app = Router::new()
        .route("/", get(pages::home))
        .nest("/auth", auth::router())
        .route("/profile", get(pages::profile))
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
    target: String,
}

async fn repo_create_run(
    _auth: AdminUser,
    Path(repo_id): Path<RepositoryId>,
    State(db): State<Db>,
    Form(data): Form<RepoCreateRun>,
) -> Result<impl IntoResponse, AppError> {
    let repository = Repository::get(&db, repo_id).await?.resource;
    let repository_host = RepositoryHost::get(&db, repository.host).await?.resource;
    match repository_host.kind {
        RepoHostKind::Github => {
            let github_app = AppClient::new(AppAuth {
                app_id: AppId(
                    repository_host
                        .app_id
                        .context("no app id")?
                        .parse()
                        .context("invalid app id")?,
                ),
                key: jsonwebtoken::EncodingKey::from_rsa_pem(
                    repository_host
                        .app_key
                        .context("no app key")?
                        .expose_secret()
                        .as_bytes(),
                )
                .context("decode github app key")?,
            });
            let installations = github_app.app_installations().await?;
            // FIXME: Using the first installation is stupid
            let installation = installations.first().context("first installation")?;
            let installation_client = github_app.auth_installation(installation.id).await?;
            let commit = installation_client
                .get_commit(&repository.owner, &repository.name, &data.commit)
                .await?;
            let run_id = Run {
                repository: repo_id,
                commit: commit.sha,
                created_at: Utc::now(),
                dequeued_at: None,
                finished: None,
                target: data.target,
                rain_version: None,
            }
            .create(&db)
            .await?;
            db::request_run(&db, run_id).await?;
            Ok(Redirect::to(&format!("/run/{run_id}")))
        }
        RepoHostKind::Forgejo => {
            let forgejo = forgejo_api::Forgejo::new(
                forgejo_api::Auth::Token(
                    repository_host.app_key.context("no token")?.expose_secret(),
                ),
                Url::parse(&repository_host.url)?,
            )?;
            let commit = forgejo
                .repo_get_single_commit(
                    &repository.owner,
                    &repository.name,
                    &data.commit,
                    forgejo_api::structs::RepoGetSingleCommitQuery::default(),
                )
                .await
                .context("forgejo get single commit")?;
            let run_id = Run {
                repository: repo_id,
                commit: commit.sha.context("no commit sha")?,
                created_at: Utc::now(),
                dequeued_at: None,
                finished: None,
                target: data.target,
                rain_version: None,
            }
            .create(&db)
            .await?;
            db::request_run(&db, run_id).await?;
            Ok(Redirect::to(&format!("/run/{run_id}")))
        }
    }
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
    db: Db,
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
    Db: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = Db::from_ref(state);

        let Some(session): Option<&session::Session> = parts.extensions.get() else {
            unreachable!("get session extension");
        };

        let user = db::get_user(&store, session.id)
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
    Db: FromRef<S>,
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
    Db: FromRef<S>,
    Arc<Config>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = Db::from_ref(state);
        let config = Arc::<Config>::from_ref(state);

        let Some(session): Option<&session::Session> = parts.extensions.get() else {
            unreachable!("get session extension");
        };
        let user = db::get_user(&store, session.id)
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
    Db: FromRef<S>,
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
