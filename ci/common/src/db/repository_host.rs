use std::str::FromStr as _;

use anyhow::Result;
use secrecy::SecretString;

use crate::db::WithId;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct RepositoryHostId(pub i64);

impl std::fmt::Display for RepositoryHostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct RepositoryHost {
    pub kind: RepoHostKind,
    pub url: String,
    pub app_id: Option<String>,
    pub app_key: Option<SecretString>,
    pub webhook_secret: Option<SecretString>,
}

#[derive(Debug, Clone, Copy, strum::IntoStaticStr, strum::EnumString, PartialEq, Eq)]
pub enum RepoHostKind {
    Github,
    Gitlab,
    Forgejo,
}

impl super::Resource for RepositoryHost {
    type Id = RepositoryHostId;

    async fn get(db: &super::Db, id: Self::Id) -> Result<WithId<Self>> {
        let row = sqlx::query!(
            "SELECT kind, url, app_id, app_key, webhook_secret FROM repo_hosts WHERE id=$1",
            id.0,
        )
        .fetch_one(&db.pool)
        .await?;
        Ok(WithId {
            id,
            resource: Self {
                kind: RepoHostKind::from_str(&row.kind)?,
                url: row.url,
                app_id: row.app_id,
                app_key: row.app_key.map(SecretString::from),
                webhook_secret: row.webhook_secret.map(SecretString::from),
            },
        })
    }
}

impl RepositoryHost {
    pub async fn create(&self, db: &super::Db) -> Result<RepositoryHostId> {
        let kind: &'static str = self.kind.into();
        let app_key = self
            .app_key
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret);
        let webhook_secret = self
            .webhook_secret
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret);
        let row = sqlx::query!(
            "INSERT INTO repo_hosts (kind, url, app_id, app_key, webhook_secret) VALUES ($1, $2, $3, $4, $5) RETURNING id",
            kind, self.url, self.app_id, app_key, webhook_secret,
        )
        .fetch_one(&db.pool)
        .await?;
        Ok(RepositoryHostId(row.id))
    }
}
