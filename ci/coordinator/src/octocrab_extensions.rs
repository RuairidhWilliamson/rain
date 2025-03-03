use octocrab::{Octocrab, Page, Result, models::hooks::Hook};

pub trait OctocrabExt {
    async fn list_hooks(&self, owner: &str, repo: &str) -> Result<Page<Hook>>;
    async fn delete_hook(&self, owner: &str, repo: &str, hook_id: u64) -> Result<()>;
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
}
