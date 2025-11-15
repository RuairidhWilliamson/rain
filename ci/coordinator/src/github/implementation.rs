use std::time::SystemTime;

use anyhow::{Context as _, Result};
use http::header::{ACCEPT, CONTENT_TYPE};
use jsonwebtoken::EncodingKey;
use serde::Serialize;
use serde_json::Value;
use ureq::{Agent, RequestBuilder};

pub struct AppAuth {
    pub app_id: super::model::AppId,
    pub key: EncodingKey,
}

impl AppAuth {
    fn generate_bearer_token(&self) -> Result<String> {
        #[derive(Serialize)]
        struct Claims {
            iss: super::model::AppId,
            iat: u64,
            exp: u64,
        }
        let now = SystemTime::UNIX_EPOCH.elapsed()?.as_secs();
        let claims = Claims {
            iss: self.app_id,
            iat: now - 60,
            exp: now + 9 * 60,
        };
        Ok(jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &self.key,
        )?)
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

    #[expect(dead_code)]
    pub fn meta(&self) -> Result<super::model::ApiOverview> {
        Ok(self
            .agent
            .get("https://api.github.com/meta")
            .call()?
            .body_mut()
            .read_json()?)
    }

    #[expect(dead_code)]
    pub fn app_installations(&self) -> Result<Vec<super::model::Installation>> {
        Ok(self
            .auth(self.agent.get("https://api.github.com/app/installations"))?
            .call()?
            .body_mut()
            .read_json()?)
    }
}

impl super::Client for AppClient {
    fn auth_installation(
        &self,
        installation_id: super::model::InstallationId,
    ) -> Result<impl super::InstallationClient> {
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
    pub token: super::model::InstallationAccessToken,
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

    #[expect(dead_code)]
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

    fn git_lfs_api(
        &self,
        owner: &str,
        repo: &str,
        request: git_lfs_rs::api::Request,
    ) -> Result<git_lfs_rs::api::Response> {
        let response = self
            .agent
            .post(format!(
                "https://github.com/{owner}/{repo}.git/info/lfs/objects/batch"
            ))
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", self.token.token),
            )
            .header(CONTENT_TYPE, "application/vnd.git-lfs+json")
            .header(ACCEPT, "application/vnd.git-lfs+json")
            .send_json(request)?;
        let response: git_lfs_rs::api::Response = response.into_body().read_json()?;
        Ok(response)
    }
}

impl super::InstallationClient for InstallationClient {
    fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run: super::model::CreateCheckRun,
    ) -> Result<super::model::CheckRun> {
        Ok(self
            .auth(self.agent.post(format!(
                "https://api.github.com/repos/{owner}/{repo}/check-runs"
            )))
            .send_json(check_run)?
            .body_mut()
            .read_json()?)
    }

    fn update_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: u64,
        check_run: super::model::PatchCheckRun,
    ) -> Result<super::model::CheckRun> {
        Ok(self
            .auth(self.agent.patch(format!(
                "https://api.github.com/repos/{owner}/{repo}/check-runs/{check_run_id}"
            )))
            .send_json(check_run)?
            .body_mut()
            .read_json()?)
    }

    fn download_repo_tar(&self, owner: &str, repo: &str, git_ref: &str) -> Result<Vec<u8>> {
        Ok(self
            .auth(self.agent.get(format!(
                "https://api.github.com/repos/{owner}/{repo}/tarball/{git_ref}"
            )))
            .call()?
            .body_mut()
            .read_to_vec()?)
    }

    fn smudge_git_lfs(
        &self,
        owner: &str,
        repo: &str,
        entries: Vec<(std::path::PathBuf, git_lfs_rs::object::Object)>,
    ) -> Result<()> {
        let request = git_lfs_rs::api::Request {
            operation: git_lfs_rs::api::Operation::Download,
            transfers: vec![git_lfs_rs::api::Transfer::Basic],
            r#ref: None,
            objects: entries.iter().map(|(_, o)| o.into()).collect(),
            hash_algo: git_lfs_rs::api::HashAlgorithm::Sha256,
        };
        let response = self
            .git_lfs_api(owner, repo, request)
            .context("git lfs api")?;
        for (resp, (path, _)) in response.objects.into_iter().zip(entries.into_iter()) {
            let mut f = std::fs::File::create(&path).context("create lfs file")?;
            let mut reader = self
                .agent
                .get(
                    &resp
                        .actions
                        .get(&git_lfs_rs::api::Operation::Download)
                        .context("no download action")?
                        .href,
                )
                .call()
                .context("download lfs object")?
                .into_body()
                .into_reader();
            std::io::copy(&mut reader, &mut f)?;
        }
        Ok(())
    }
}
