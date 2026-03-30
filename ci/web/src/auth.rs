use std::sync::Arc;

use anyhow::{Context as _, Result};
use axum::extract::Path;
use axum::http::header::{ACCEPT, USER_AGENT};
use axum::{
    Extension, Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    routing::get,
};
use oauth2::Scope;
use oauth2::{
    AccessToken, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    EmptyExtraTokenFields, EndpointNotSet, EndpointSet, RedirectUrl, StandardTokenResponse,
    TokenResponse as _, TokenUrl, basic::BasicTokenType, url::Url,
};
use rain_ci_common::db::Db;
use secrecy::ExposeSecret as _;
use serde::Deserialize;

use crate::db::{AuthKind, AuthProvider};
use crate::{AppError, AppState, Config, session};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(default_auth))
        .route("/{name}", get(auth))
        .route("/{name}/authorized", get(authorized))
}

async fn default_auth(State(config): State<Arc<Config>>) -> impl IntoResponse {
    Redirect::to(&format!("/auth/{}", config.default_auth))
}

async fn auth(
    Path(name): Path<String>,
    State(db): State<Db>,
    State(config): State<Arc<Config>>,
    Extension(session): Extension<session::Session>,
) -> Result<impl IntoResponse, AppError> {
    let auth_provider = crate::db::get_auth_provider(&db, &name).await?;
    let client = Client::new_from_auth_provider(&config.base_url, auth_provider).await?;
    let (auth_url, csrf_token) = client.authorize_url();
    crate::db::set_session_csrf(&db, &session.id, csrf_token)
        .await
        .context("set session csrf")?;
    Ok(Redirect::to(auth_url.as_ref()))
}

#[derive(Debug, serde::Deserialize)]
struct AuthRequest {
    code: AuthorizationCode,
    state: CsrfToken,
}

async fn authorized(
    Path(name): Path<String>,
    Query(query): Query<AuthRequest>,
    State(db): State<Db>,
    State(config): State<Arc<Config>>,
    Extension(session): Extension<session::Session>,
) -> Result<impl IntoResponse, AppError> {
    let auth_provider = crate::db::get_auth_provider(&db, &name)
        .await
        .context("get auth provider")?;
    let client = Client::new_from_auth_provider(&config.base_url, auth_provider)
        .await
        .context("new auth provider client")?;
    crate::db::check_session_csrf(&db, &session.id, query.state)
        .await
        .map_err(|err| anyhow::format_err!("csrf check failed: {err:#}"))?;
    let token = client.exchange_code(query.code).await?;
    let user = client
        .get_user_details(token.access_token())
        .await
        .context("get user details")?;
    crate::db::auth_user_session(&db, &session.id, &name, user)
        .await
        .map_err(|err| anyhow::format_err!("auth user session: {err:#}"))?;
    Ok(Redirect::to("/"))
}

type BasicClient = oauth2::basic::BasicClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

pub type OAuthToken = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

#[derive(Clone)]
pub struct Client {
    kind: AuthKind,
    oauth: BasicClient,
    user_info_endpoint: Option<Url>,
}

impl Client {
    pub async fn new_from_auth_provider(
        base_url: &str,
        auth_provider: AuthProvider,
    ) -> Result<Self> {
        let redirect_url = format!(
            "{base_url}/auth/{name}/authorized",
            name = auth_provider.name
        );
        match auth_provider.kind {
            crate::db::AuthKind::Github => {
                let auth_url =
                    "https://github.com/login/oauth/authorize?response_type=code".to_string();
                let token_url = "https://github.com/login/oauth/access_token".to_string();
                let oauth_client =
                    oauth2::basic::BasicClient::new(ClientId::new(auth_provider.client_id.clone()))
                        .set_client_secret(ClientSecret::new(
                            auth_provider.client_secret.expose_secret().to_owned(),
                        ))
                        .set_auth_uri(
                            AuthUrl::new(auth_url)
                                .context("failed to create new authorization server URL")?,
                        )
                        .set_token_uri(
                            TokenUrl::new(token_url)
                                .context("failed to create new token endpoint URL")?,
                        )
                        .set_redirect_uri(
                            RedirectUrl::new(redirect_url)
                                .context("failed to create new redirection URL")?,
                        );

                Ok(Self {
                    kind: auth_provider.kind,
                    oauth: oauth_client,
                    user_info_endpoint: None,
                })
            }
            crate::db::AuthKind::OpenIDConnect => {
                let discovery_url = auth_provider
                    .oidc_discovery_url
                    .as_ref()
                    .context("no oidc discovery url")?;
                let discovered: OpenIDConnectDiscovered = http_client()?
                    .get(discovery_url)
                    .send()
                    .await?
                    .json()
                    .await?;

                let oauth_client =
                    oauth2::basic::BasicClient::new(ClientId::new(auth_provider.client_id.clone()))
                        .set_client_secret(ClientSecret::new(
                            auth_provider.client_secret.expose_secret().to_owned(),
                        ))
                        .set_auth_uri(
                            AuthUrl::new(discovered.authorization_endpoint)
                                .context("failed to create new authorization server URL")?,
                        )
                        .set_token_uri(
                            TokenUrl::new(discovered.token_endpoint.context("no token endpoint")?)
                                .context("failed to create new token endpoint URL")?,
                        )
                        .set_redirect_uri(
                            RedirectUrl::new(redirect_url)
                                .context("failed to create new redirection URL")?,
                        );

                Ok(Self {
                    kind: auth_provider.kind,
                    oauth: oauth_client,
                    user_info_endpoint: Some(Url::parse(
                        &discovered
                            .userinfo_endpoint
                            .context("no user info endpoint")?,
                    )?),
                })
            }
        }
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken) {
        let (auth_url, csrf_token) = self
            .oauth
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(String::from("openid")))
            .add_scope(Scope::new(String::from("email")))
            .add_scope(Scope::new(String::from("profile")))
            .url();
        (auth_url, csrf_token)
    }

    pub async fn exchange_code(&self, code: AuthorizationCode) -> Result<OAuthToken> {
        let token = self
            .oauth
            .exchange_code(code)
            .request_async(&http_client()?)
            .await?;
        Ok(token)
    }

    pub async fn get_user_details(&self, token: &AccessToken) -> Result<UserDetails> {
        match self.kind {
            AuthKind::Github => {
                let http_client = reqwest::Client::builder().build()?;
                let response = http_client
                    .get("https://api.github.com/user")
                    .bearer_auth(token.secret())
                    .header(ACCEPT, "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28")
                    .header(USER_AGENT, "RainCIWeb")
                    .send()
                    .await?;
                let body = response.bytes().await?;
                let user: GithubUserDetails =
                    serde_json::from_slice(&body).context("deserialize json user")?;
                if let Some(email) = user.email {
                    return Ok(UserDetails {
                        name: user.name,
                        email,
                        login: user.login,
                        preferred_username: user.preferred_username,
                        avatar_url: user.avatar_url,
                    });
                }
                // Github user email is only provided if the user's email is public
                // Otherwise we have to call /user/emails to get their email address
                let response = http_client
                    .get("https://api.github.com/user/emails")
                    .bearer_auth(token.secret())
                    .header(ACCEPT, "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28")
                    .header(USER_AGENT, "RainCIWeb")
                    .send()
                    .await?;
                let body = response.bytes().await?;
                let emails: Vec<GithubEmail> =
                    serde_json::from_slice(&body).context("deserialize json github email")?;
                let email = emails
                    .into_iter()
                    .find(|email| email.primary && email.verified)
                    .context("no primary email that is verified")?;
                Ok(UserDetails {
                    name: user.name,
                    email: email.email,
                    login: user.login,
                    preferred_username: user.preferred_username,
                    avatar_url: user.avatar_url,
                })
            }
            AuthKind::OpenIDConnect => {
                let response = http_client()?
                    .get(
                        self.user_info_endpoint
                            .clone()
                            .context("no user info endpoint")?,
                    )
                    .bearer_auth(token.secret())
                    .header(ACCEPT, "application/json")
                    .header(USER_AGENT, "RainCIWeb")
                    .send()
                    .await?;
                let body = response.bytes().await?;
                Ok(serde_json::from_slice(&body)?)
            }
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct GithubUserDetails {
    pub name: String,
    pub email: Option<String>,
    pub login: Option<String>,
    pub preferred_username: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct GithubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

#[derive(Clone, Deserialize)]
pub struct UserDetails {
    pub name: String,
    pub email: String,
    pub login: Option<String>,
    #[expect(dead_code)]
    pub preferred_username: Option<String>,
    pub avatar_url: Option<String>,
}

#[expect(clippy::struct_field_names)]
#[derive(Debug, Deserialize)]
pub struct OpenIDConnectDiscovered {
    pub authorization_endpoint: String,
    pub token_endpoint: Option<String>,
    pub userinfo_endpoint: Option<String>,
}

fn http_client() -> Result<reqwest::Client> {
    let http_client = reqwest::Client::builder().build()?;
    Ok(http_client)
}
