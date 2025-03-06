use octocrab::{Octocrab, Page, Result, models::hooks::Hook};
use serde::{Deserialize, Serialize};

pub trait OctocrabExt {
    async fn list_hooks(&self, owner: &str, repo: &str) -> Result<Page<Hook>>;
    async fn delete_hook(&self, owner: &str, repo: &str, hook_id: u64) -> Result<()>;
    async fn get_tree(&self, owner: &str, repo: &str, sha: &str) -> Result<TreesResponse>;
    async fn get_blob(&self, owner: &str, repo: &str, file_sha: &str) -> Result<BlobResponse>;
}

impl OctocrabExt for Octocrab {
    async fn list_hooks(&self, owner: &str, repo: &str) -> Result<Page<Hook>> {
        self.get(format!("/repos/{owner}/{repo}/hooks"), None::<&()>)
            .await
    }

    async fn delete_hook(&self, owner: &str, repo: &str, hook_id: u64) -> Result<()> {
        self._delete(
            format!("/repos/{owner}/{repo}/hooks/{hook_id}"),
            None::<&()>,
        )
        .await?;
        Ok(())
    }

    async fn get_tree(&self, owner: &str, repo: &str, sha: &str) -> Result<TreesResponse> {
        self.get(
            format!("/repos/{owner}/{repo}/git/trees/{sha}"),
            None::<&()>,
        )
        .await
    }

    async fn get_blob(&self, owner: &str, repo: &str, file_sha: &str) -> Result<BlobResponse> {
        self.get(
            format!("/repos/{owner}/{repo}/git/blobs/{file_sha}"),
            None::<&()>,
        )
        .await
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreesResponse {
    pub sha: String,
    pub url: String,
    pub tree: Vec<TreeEntry>,
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum TreeEntry {
    Blob {
        #[serde(flatten)]
        blob: Blob,
    },
    Tree {
        #[serde(flatten)]
        tree: Tree,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Blob {
    pub path: String,
    pub mode: String,
    pub size: u64,
    pub sha: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tree {
    pub path: String,
    pub mode: String,
    pub sha: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlobResponse {
    pub content: String,
    pub encoding: String,
    pub url: String,
    pub sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlighted_content: Option<String>,
}
