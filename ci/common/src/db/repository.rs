use anyhow::Result;

use crate::{
    db::{
        Resource as _, WithId,
        repository_host::{RepoHostKind, RepositoryHost, RepositoryHostId},
    },
    pagination::{Paginated, Pagination},
};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct RepositoryId(pub i64);

impl std::fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub host: RepositoryHostId,
    pub owner: String,
    pub name: String,
}

impl super::Resource for Repository {
    type Id = RepositoryId;

    async fn get(db: &super::Db, id: RepositoryId) -> Result<WithId<Self>> {
        let repo = sqlx::query!("SELECT host, owner, name FROM repos WHERE id=$1", id.0,)
            .fetch_one(&db.pool)
            .await?;
        Ok(WithId {
            id,
            resource: Self {
                host: RepositoryHostId(repo.host),
                owner: repo.owner,
                name: repo.name,
            },
        })
    }
}

impl Repository {
    pub async fn create(&self, db: &super::Db) -> Result<RepositoryId> {
        let row = sqlx::query!(
            "INSERT INTO repos (host, owner, name) VALUES ($1, $2, $3) RETURNING id",
            self.host.0,
            self.owner,
            self.name,
        )
        .fetch_one(&db.pool)
        .await?;
        Ok(RepositoryId(row.id))
    }

    pub async fn resolve(self, db: &super::Db) -> Result<ResolvedRepository> {
        Ok(ResolvedRepository {
            host: RepositoryHost::get(db, self.host).await?,
            owner: self.owner,
            name: self.name,
        })
    }

    pub async fn find(&self, db: &super::Db) -> Result<RepositoryId> {
        let row = sqlx::query!(
            "SELECT id FROM repos WHERE host=$1 AND owner=$2 and name=$3",
            self.host.0,
            self.owner,
            self.name
        )
        .fetch_one(&db.pool)
        .await?;
        Ok(RepositoryId(row.id))
    }

    pub async fn list(db: &super::Db, page: &Pagination) -> Result<Paginated<WithId<Self>>> {
        let mut tx = db.pool.begin().await?;
        let per_page = i64::try_from(page.per_page())?;
        let rows = sqlx::query_as!(
            QueryRepository,
            r#"
            SELECT
                id,
                host,
                owner,
                name
            FROM repos
            OFFSET $1 LIMIT $2;
            "#,
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count_row = sqlx::query!("SELECT COUNT(*) FROM repos")
            .fetch_one(&mut *tx)
            .await?;

        tx.rollback().await?;

        let elements: Vec<WithId<Self>> = rows.into_iter().map(QueryRepository::convert).collect();
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(elements, full_count, page.per_page(), page))
    }

    pub fn external_repo_url(&self, host: &RepositoryHost) -> String {
        match host.kind {
            RepoHostKind::Github | RepoHostKind::Gitlab | RepoHostKind::Forgejo => format!(
                "{url}/{owner}/{name}",
                url = host.url,
                owner = self.owner,
                name = self.name,
            ),
        }
    }
}

struct QueryRepository {
    id: i64,
    host: i64,
    owner: String,
    name: String,
}

impl QueryRepository {
    fn convert(self) -> WithId<Repository> {
        WithId {
            id: RepositoryId(self.id),
            resource: Repository {
                host: RepositoryHostId(self.host),
                owner: self.owner,
                name: self.name,
            },
        }
    }
}

pub struct ResolvedRepository {
    pub host: WithId<RepositoryHost>,
    pub owner: String,
    pub name: String,
}

impl super::Resource for ResolvedRepository {
    type Id = RepositoryId;

    async fn get(db: &super::Db, id: RepositoryId) -> Result<WithId<Self>> {
        let repo = sqlx::query!("SELECT host, owner, name FROM repos WHERE id=$1", id.0,)
            .fetch_one(&db.pool)
            .await?;
        Ok(WithId {
            id,
            resource: Self {
                host: RepositoryHost::get(db, RepositoryHostId(repo.host)).await?,
                owner: repo.owner,
                name: repo.name,
            },
        })
    }
}

impl ResolvedRepository {
    pub async fn list(db: &super::Db, page: &Pagination) -> Result<Paginated<WithId<Self>>> {
        let mut tx = db.pool.begin().await?;
        let per_page = i64::try_from(page.per_page())?;
        let rows = sqlx::query_as!(
            QueryRepository,
            r#"
            SELECT
                id,
                host,
                owner,
                name
            FROM repos
            OFFSET $1 LIMIT $2;
            "#,
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count_row = sqlx::query!("SELECT COUNT(*) FROM repos")
            .fetch_one(&mut *tx)
            .await?;

        tx.rollback().await?;

        let elements: Vec<WithId<Repository>> =
            rows.into_iter().map(QueryRepository::convert).collect();
        let mut repos: Vec<WithId<Self>> = Vec::with_capacity(elements.len());
        for repo in elements {
            let resolved = repo.resource.resolve(db).await?;
            repos.push(WithId {
                id: repo.id,
                resource: resolved,
            });
        }
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(repos, full_count, page.per_page(), page))
    }

    pub fn external_repo_url(&self) -> String {
        match self.host.resource.kind {
            RepoHostKind::Github | RepoHostKind::Gitlab | RepoHostKind::Forgejo => format!(
                "{url}/{owner}/{name}",
                url = self.host.resource.url,
                owner = self.owner,
                name = self.name,
            ),
        }
    }
}
