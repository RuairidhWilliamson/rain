#![allow(dead_code, clippy::unwrap_used)]

pub mod model;

use std::time::SystemTime;

use anyhow::Result;
use jsonwebtoken::EncodingKey;
use serde::Serialize;
use serde_json::Value;
use ureq::{Agent, RequestBuilder};

pub struct AppAuth {
    pub app_id: model::AppId,
    pub key: EncodingKey,
}

impl AppAuth {
    fn generate_bearer_token(&self) -> jsonwebtoken::errors::Result<String> {
        #[derive(Serialize)]
        struct Claims {
            iss: model::AppId,
            iat: u64,
            exp: u64,
        }
        let now = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs();
        let claims = Claims {
            iss: self.app_id,
            iat: now - 60,
            exp: now + 9 * 60,
        };
        jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &self.key,
        )
    }
}

pub struct AppClient {
    auth: AppAuth,
    agent: ureq::Agent,
}

impl AppClient {
    pub fn new(auth: AppAuth) -> Self {
        let config = Agent::config_builder().https_only(true).build();
        let agent = Agent::new_with_config(config);
        Self { auth, agent }
    }

    fn auth<Any>(&self, req: RequestBuilder<Any>) -> Result<RequestBuilder<Any>> {
        Ok(req
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", self.auth.generate_bearer_token()?),
            )
            .header(http::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28"))
    }

    pub fn meta(&self) -> Result<model::ApiOverview> {
        Ok(self
            .agent
            .get("https://api.github.com/meta")
            .call()?
            .body_mut()
            .read_json()?)
    }

    pub fn app_installations(&self) -> Result<Vec<model::Installation>> {
        Ok(self
            .auth(self.agent.get("https://api.github.com/app/installations"))?
            .call()?
            .body_mut()
            .read_json()?)
    }

    pub fn auth_installation(
        &self,
        installation_id: model::InstallationId,
    ) -> Result<InstallationClient> {
        let token = self
            .auth(self.agent.post(format!(
                "https://api.github.com/app/installations/{installation_id}/access_tokens"
            )))?
            .send_empty()?
            .body_mut()
            .read_json()?;
        Ok(InstallationClient {
            agent: self.agent.clone(),
            token,
        })
    }
}

pub struct InstallationClient {
    pub agent: ureq::Agent,
    pub token: model::InstallationAccessToken,
}

impl InstallationClient {
    pub fn auth<Any>(&self, req: RequestBuilder<Any>) -> RequestBuilder<Any> {
        req.header(
            http::header::AUTHORIZATION,
            format!("Bearer {}", self.token.token),
        )
        .header(http::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
    }

    pub fn app_installation_repositories(&self) -> Result<Value> {
        Ok(self
            .auth(
                self.agent
                    .get("https://api.github.com/installation/repositories"),
            )
            .call()?
            .body_mut()
            .read_json()?)
    }

    pub fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run: model::CreateCheckRun,
    ) -> Result<model::CheckRun> {
        Ok(self
            .auth(self.agent.post(format!(
                "https://api.github.com/repos/{owner}/{repo}/check-runs"
            )))
            .send_json(check_run)?
            .body_mut()
            .read_json()?)
    }

    pub fn update_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: u64,
        check_run: model::PatchCheckRun,
    ) -> Result<model::CheckRun> {
        Ok(self
            .auth(self.agent.patch(format!(
                "https://api.github.com/repos/{owner}/{repo}/check-runs/{check_run_id}"
            )))
            .send_json(check_run)?
            .body_mut()
            .read_json()?)
    }

    pub fn download_repo_tar(&self, owner: &str, repo: &str, git_ref: &str) -> Result<Vec<u8>> {
        Ok(self
            .auth(self.agent.get(format!(
                "https://api.github.com/repos/{owner}/{repo}/tarball/{git_ref}"
            )))
            .call()?
            .body_mut()
            .read_to_vec()?)
    }
}
