use anyhow::{Context as _, Result};
use axum::http::header::{ACCEPT, USER_AGENT};
use oauth2::{
    AccessToken, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    EmptyExtraTokenFields, EndpointNotSet, EndpointSet, RedirectUrl, StandardTokenResponse,
    TokenUrl,
    basic::BasicTokenType,
    reqwest::{self, redirect::Policy},
    url::Url,
};

type BasicClient = oauth2::basic::BasicClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UserDetails {
    pub id: i64,
    pub login: String,
    pub name: String,
    pub avatar_url: String,
}

pub type OAuthToken = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

#[derive(Clone)]
pub struct Client {
    oauth_client: BasicClient,
}

impl Client {
    pub fn new(client_id: String, client_secret: ClientSecret, base_url: &str) -> Result<Self> {
        let redirect_url = format!("{base_url}/auth/authorized");
        let auth_url = "https://github.com/login/oauth/authorize?response_type=code".to_string();
        let token_url = "https://github.com/login/oauth/access_token".to_string();
        let oauth_client = oauth2::basic::BasicClient::new(ClientId::new(client_id))
            .set_client_secret(client_secret)
            .set_auth_uri(
                AuthUrl::new(auth_url).context("failed to create new authorization server URL")?,
            )
            .set_token_uri(
                TokenUrl::new(token_url).context("failed to create new token endpoint URL")?,
            )
            .set_redirect_uri(
                RedirectUrl::new(redirect_url).context("failed to create new redirection URL")?,
            );

        Ok(Self { oauth_client })
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken) {
        let (auth_url, csrf_token) = self
            .oauth_client
            .authorize_url(CsrfToken::new_random)
            // .add_scope(Scope::new("user:email".to_string()))
            .url();
        (auth_url, csrf_token)
    }

    pub async fn exchange_code(&self, code: AuthorizationCode) -> Result<OAuthToken> {
        let http_client = reqwest::Client::builder()
            .redirect(Policy::none())
            .build()?;
        let token = self
            .oauth_client
            .exchange_code(code)
            .request_async(&http_client)
            .await?;
        Ok(token)
    }

    pub async fn get_user_details(&self, token: &AccessToken) -> Result<UserDetails> {
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
        Ok(serde_json::from_slice(&body)?)
    }
}
