use std::time::SystemTime;

use anyhow::{Context as _, Result};
use http::header::CONTENT_TYPE;
use jsonwebtoken::EncodingKey;
use serde::Serialize;
use serde_json::Value;

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
    client: reqwest::Client,
    auth: AppAuth,
}

impl AppClient {
    pub fn new(auth: AppAuth) -> Self {
        let client = reqwest::ClientBuilder::new()
            .https_only(true)
            .user_agent("rain-ci")
            .build()
            .expect("build client");
        Self { client, auth }
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        Ok(req
            .bearer_auth(self.auth.generate_bearer_token()?)
            .header(http::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28"))
    }

    pub async fn meta(&self) -> Result<super::model::ApiOverview> {
        Ok(self
            .client
            .get("https://api.github.com/meta")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

impl super::Client for AppClient {
    async fn auth_installation(
        &self,
        installation_id: super::model::InstallationId,
    ) -> Result<impl super::InstallationClient> {
        let token = self
            .auth(self.client.post(format!(
                "https://api.github.com/app/installations/{installation_id}/access_tokens"
            )))?
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(InstallationClient {
            client: self.client.clone(),
            token,
        })
    }

    async fn app_installations(&self) -> Result<Vec<super::model::Installation>> {
        Ok(self
            .auth(self.client.get("https://api.github.com/app/installations"))?
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

pub struct InstallationClient {
    pub client: reqwest::Client,
    pub token: super::model::InstallationAccessToken,
}

impl InstallationClient {
    pub fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header(
            http::header::AUTHORIZATION,
            format!("Bearer {}", self.token.token),
        )
        .header(http::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
    }

    pub async fn app_installation_repositories(&self) -> Result<Value> {
        Ok(self
            .auth(
                self.client
                    .get("https://api.github.com/installation/repositories"),
            )
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn git_lfs_api(
        &self,
        owner: &str,
        repo: &str,
        request: git_lfs_rs::api::Request,
    ) -> Result<git_lfs_rs::api::Response> {
        let response = self
            .auth(self.client.post(format!(
                "https://github.com/{owner}/{repo}.git/info/lfs/objects/batch"
            )))
            .header(CONTENT_TYPE, "application/vnd.git-lfs+json")
            .json(&request)
            .send()
            .await?
            .error_for_status()?;
        let response: git_lfs_rs::api::Response = response.json().await?;
        Ok(response)
    }
}

impl super::InstallationClient for InstallationClient {
    async fn get_repo(&self, owner: &str, repo: &str) -> Result<super::model::Repository> {
        Ok(self
            .auth(
                self.client
                    .get(format!("https://api.github.com/repos/{owner}/{repo}")),
            )
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn get_commit(
        &self,
        owner: &str,
        repo: &str,
        r#ref: &str,
    ) -> Result<super::model::Commit> {
        Ok(self
            .auth(self.client.get(format!(
                "https://api.github.com/repos/{owner}/{repo}/commits/{ref}"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run: super::model::CreateCheckRun,
    ) -> Result<super::model::CheckRun> {
        Ok(self
            .client
            .post(format!(
                "https://api.github.com/repos/{owner}/{repo}/check-runs"
            ))
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", self.token.token),
            )
            .header(http::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&check_run)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn update_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: u64,
        check_run: super::model::PatchCheckRun,
    ) -> Result<super::model::CheckRun> {
        Ok(self
            .auth(self.client.patch(format!(
                "https://api.github.com/repos/{owner}/{repo}/check-runs/{check_run_id}"
            )))
            .json(&check_run)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn download_repo_tar(&self, owner: &str, repo: &str, git_ref: &str) -> Result<Vec<u8>> {
        Ok(self
            .auth(self.client.get(format!(
                "https://api.github.com/repos/{owner}/{repo}/tarball/{git_ref}"
            )))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .to_vec())
    }

    async fn smudge_git_lfs(
        &self,
        owner: &str,
        repo: &str,
        entries: Vec<(std::path::PathBuf, git_lfs_rs::object::Object)>,
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let request = git_lfs_rs::api::Request {
            operation: git_lfs_rs::api::Operation::Download,
            transfers: vec![git_lfs_rs::api::Transfer::Basic],
            r#ref: None,
            objects: entries.iter().map(|(_, o)| o.into()).collect(),
            hash_algo: git_lfs_rs::api::HashAlgorithm::Sha256,
        };
        let response = self
            .git_lfs_api(owner, repo, request)
            .await
            .context("git lfs api")?;
        for (resp, (path, _)) in response.objects.into_iter().zip(entries.into_iter()) {
            let mut f = tokio::fs::File::create(&path)
                .await
                .context("create lfs file")?;
            let body = self
                .client
                .get(
                    &resp
                        .actions
                        .get(&git_lfs_rs::api::Operation::Download)
                        .context("no download action")?
                        .href,
                )
                .send()
                .await
                .context("download lfs object")?
                .error_for_status()?
                .bytes()
                .await?;
            let mut reader = &body[..];
            tokio::io::copy(&mut reader, &mut f).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::EncodingKey;

    use crate::github::{Client as _, InstallationClient as _};

    // Requires github creds
    #[ignore]
    #[tokio::test]
    async fn create_check_run() {
        dotenvy::dotenv().unwrap();
        let id =
            super::super::model::AppId(std::env::var("GITHUB_APP_ID").unwrap().parse().unwrap());
        let key_file = std::env::var("GITHUB_APP_KEY_FILE").unwrap();
        let key_raw = tokio::fs::read(key_file).await.unwrap();
        let key = EncodingKey::from_rsa_pem(&key_raw).unwrap();
        let client = super::AppClient::new(super::AppAuth { app_id: id, key });
        let installations = client.app_installations().await.unwrap();
        dbg!(&installations);
        let installation = installations.first().unwrap();
        let installation_client = client.auth_installation(installation.id).await.unwrap();
        installation_client
            .create_check_run(
                "RuairidhWilliamson",
                "rain",
                crate::github::model::CreateCheckRun {
                    name: String::from("test"),
                    head_sha: String::from("518b1e599c940946200e92d1f3d84b05fbb3d840"),
                    status: crate::github::model::Status::Queued,
                    details_url: None,
                    output: None,
                },
            )
            .await
            .unwrap();
    }
}
