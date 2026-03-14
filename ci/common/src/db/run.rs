use std::str::FromStr as _;

use anyhow::{Context as _, Result};
use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};

use crate::{
    db::{
        Resource as _, WithId,
        repository::{Repository, RepositoryId, ResolvedRepository},
        repository_host::RepositoryHost,
    },
    pagination::{Paginated, Pagination},
};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct RunId(pub i64);

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Run {
    pub repository: RepositoryId,
    pub commit: String,
    pub created_at: DateTime<Utc>,
    pub dequeued_at: Option<DateTime<Utc>>,
    pub finished: Option<FinishedRun>,
    pub target: String,
    pub rain_version: Option<String>,
}

impl super::Resource for Run {
    type Id = RunId;

    async fn get(db: &super::Db, id: RunId) -> Result<WithId<Self>> {
        let row = sqlx::query_as!(
            QueryRun,
            r#"
                SELECT
                    id,
                    repo as repo_id,
                    commit,
                    created_at,
                    dequeued_at,
                    rain_version,
                    target,
                    finished_at as "finished_at?",
                    status as "status?",
                    execution_time_millis as "execution_time_millis?",
                    output as "output?"
                FROM runs
                LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
                WHERE runs.id=$1;
                "#,
            id.0
        )
        .fetch_one(&db.pool)
        .await?;
        row.convert()
    }
}

impl Run {
    pub async fn create(&self, db: &super::Db) -> Result<RunId> {
        let row = sqlx::query!(
                "INSERT INTO runs (created_at, repo, commit, target) VALUES ($1, $2, $3, $4) RETURNING id",
                self.created_at.naive_utc(),
                self.repository.0,
                &self.commit,
                &self.target,
            )
            .fetch_one(&db.pool)
            .await?;
        Ok(RunId(row.id))
    }

    pub async fn list_in_repo(
        db: &super::Db,
        page: &Pagination,
        repo: RepositoryId,
    ) -> Result<Paginated<WithId<Self>>> {
        let mut tx = db.pool.begin().await?;
        let per_page = i64::try_from(page.per_page())?;
        let rows = sqlx::query_as!(
            QueryRun,
            r#"
            SELECT
                runs.id,
                commit,
                created_at,
                dequeued_at,
                rain_version,
                repo AS repo_id,
                target,
                finished_at AS "finished_at?",
                status AS "status?",
                execution_time_millis AS "execution_time_millis?",
                output AS "output?"
            FROM runs
            LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
            WHERE repo=$1
            ORDER BY runs.id DESC
            OFFSET $2 LIMIT $3;
            "#,
            repo.0,
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count_row =
            sqlx::query!("SELECT COUNT(*) FROM runs INNER JOIN repos ON runs.repo=repos.id")
                .fetch_one(&mut *tx)
                .await?;

        tx.rollback().await?;

        let elements: Vec<WithId<Self>> = rows
            .into_iter()
            .map(QueryRun::convert)
            .collect::<Result<_>>()?;
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(elements, full_count, page.per_page(), page))
    }

    pub async fn list(db: &super::Db, page: &Pagination) -> Result<Paginated<WithId<Self>>> {
        let mut tx = db.pool.begin().await?;
        let per_page = i64::try_from(page.per_page())?;
        let rows = sqlx::query_as!(
            QueryRun,
            r#"
            SELECT
                runs.id,
                commit,
                created_at,
                dequeued_at,
                rain_version,
                repo AS repo_id,
                target,
                finished_at AS "finished_at?",
                status AS "status?",
                execution_time_millis AS "execution_time_millis?",
                output AS "output?"
            FROM runs
            LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
            ORDER BY runs.id DESC
            OFFSET $1 LIMIT $2;
            "#,
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count_row =
            sqlx::query!("SELECT COUNT(*) FROM runs INNER JOIN repos ON runs.repo=repos.id")
                .fetch_one(&mut *tx)
                .await?;

        tx.rollback().await?;

        let elements: Vec<WithId<Self>> = rows
            .into_iter()
            .map(QueryRun::convert)
            .collect::<Result<_>>()?;
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(elements, full_count, page.per_page(), page))
    }

    pub async fn dequeued(db: &super::Db, id: RunId, rain_version: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE runs SET dequeued_at=$1, rain_version=$2 WHERE id=$3",
            &Utc::now().naive_utc(),
            rain_version,
            id.0,
        )
        .execute(&db.pool)
        .await?;
        Ok(())
    }

    pub async fn finished(db: &super::Db, id: RunId, finished: FinishedRun) -> Result<()> {
        let run_status: &str = finished.status.into();
        sqlx::query!("INSERT INTO finished_runs (run, finished_at, status, execution_time_millis, output) VALUES ($1, $2, $3, $4, $5)", id.0, finished.finished_at.naive_utc(), run_status, finished.execution_time.num_milliseconds(), &finished.output).execute(&db.pool).await?;
        Ok(())
    }

    pub fn state(&self) -> RunState {
        if self.finished.is_some() {
            RunState::Finished
        } else if self.dequeued_at.is_some() {
            RunState::InProgress
        } else {
            RunState::Queued
        }
    }

    pub fn commit_url(&self, repository: &Repository, host: &RepositoryHost) -> String {
        format!(
            "{repo_url}/commit/{commit}",
            repo_url = repository.external_repo_url(host),
            commit = self.commit,
        )
    }

    pub async fn resolve(self, db: &super::Db) -> Result<ResolvedRun> {
        Ok(ResolvedRun {
            repository: ResolvedRepository::get(db, self.repository).await?,
            commit: self.commit,
            created_at: self.created_at,
            dequeued_at: self.dequeued_at,
            finished: self.finished,
            target: self.target,
            rain_version: self.rain_version,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FinishedRun {
    pub finished_at: DateTime<Utc>,
    pub status: RunStatus,
    pub execution_time: TimeDelta,
    pub output: String,
}

#[derive(Debug, Clone, strum::IntoStaticStr, strum::EnumString)]
pub enum RunStatus {
    Success,
    Failure,
    SystemFailure,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.into())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RunState {
    Queued,
    InProgress,
    Finished,
}

impl std::fmt::Display for RunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

struct QueryRun {
    id: i64,
    repo_id: i64,
    commit: String,
    created_at: NaiveDateTime,
    target: String,
    dequeued_at: Option<NaiveDateTime>,
    rain_version: Option<String>,
    status: Option<String>,
    finished_at: Option<NaiveDateTime>,
    execution_time_millis: Option<i64>,
    output: Option<String>,
}

impl QueryRun {
    fn convert(self) -> Result<WithId<Run>> {
        let row = self;
        Ok(WithId {
            id: RunId(row.id),
            resource: Run {
                commit: row.commit,
                created_at: row.created_at.and_utc(),
                dequeued_at: row.dequeued_at.map(|dt| dt.and_utc()),
                rain_version: row.rain_version,
                target: row.target,
                finished: row
                    .finished_at
                    .map(|finished_at| {
                        Result::<_>::Ok(FinishedRun {
                            finished_at: finished_at.and_utc(),
                            status: RunStatus::from_str(&row.status.context("status missing")?)
                                .context("unknown run status")?,
                            execution_time: TimeDelta::milliseconds(
                                row.execution_time_millis
                                    .context("execution_time_millis missing")?,
                            ),
                            output: row.output.context("output missing")?,
                        })
                    })
                    .transpose()?,
                repository: RepositoryId(row.repo_id),
            },
        })
    }
}

pub struct ResolvedRun {
    pub repository: WithId<ResolvedRepository>,
    pub commit: String,
    pub created_at: DateTime<Utc>,
    pub dequeued_at: Option<DateTime<Utc>>,
    pub finished: Option<FinishedRun>,
    pub target: String,
    pub rain_version: Option<String>,
}

impl super::Resource for ResolvedRun {
    type Id = RunId;

    async fn get(db: &super::Db, id: RunId) -> Result<WithId<Self>> {
        let run = Run::get(db, id).await?;
        Ok(WithId {
            id: run.id,
            resource: run.resource.resolve(db).await?,
        })
    }
}

impl ResolvedRun {
    pub async fn list(db: &super::Db, page: &Pagination) -> Result<Paginated<WithId<Self>>> {
        let mut tx = db.pool.begin().await?;
        let per_page = i64::try_from(page.per_page())?;
        let rows = sqlx::query_as!(
            QueryRun,
            r#"
            SELECT
                runs.id,
                commit,
                created_at,
                dequeued_at,
                rain_version,
                repo AS repo_id,
                target,
                finished_at AS "finished_at?",
                status AS "status?",
                execution_time_millis AS "execution_time_millis?",
                output AS "output?"
            FROM runs
            LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
            ORDER BY runs.id DESC
            OFFSET $1 LIMIT $2;
            "#,
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count_row =
            sqlx::query!("SELECT COUNT(*) FROM runs INNER JOIN repos ON runs.repo=repos.id")
                .fetch_one(&mut *tx)
                .await?;

        tx.rollback().await?;
        let mut elements = Vec::with_capacity(rows.len());
        for row in rows {
            let run = row.convert()?;
            let resolved = run.resource.resolve(db).await?;
            elements.push(WithId {
                id: run.id,
                resource: resolved,
            });
        }
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(elements, full_count, page.per_page(), page))
    }

    pub async fn list_in_repo(
        db: &super::Db,
        page: &Pagination,
        repo: RepositoryId,
    ) -> Result<Paginated<WithId<Self>>> {
        let mut tx = db.pool.begin().await?;
        let per_page = i64::try_from(page.per_page())?;
        let rows = sqlx::query_as!(
            QueryRun,
            r#"
            SELECT
                runs.id,
                commit,
                created_at,
                dequeued_at,
                rain_version,
                repo AS repo_id,
                target,
                finished_at AS "finished_at?",
                status AS "status?",
                execution_time_millis AS "execution_time_millis?",
                output AS "output?"
            FROM runs
            LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
            WHERE repo=$1
            ORDER BY runs.id DESC
            OFFSET $2 LIMIT $3;
            "#,
            repo.0,
            page.page_numberz()? * per_page,
            per_page,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count_row =
            sqlx::query!("SELECT COUNT(*) FROM runs INNER JOIN repos ON runs.repo=repos.id")
                .fetch_one(&mut *tx)
                .await?;

        tx.rollback().await?;

        let mut elements = Vec::with_capacity(rows.len());
        for row in rows {
            let run = row.convert()?;
            let resolved = run.resource.resolve(db).await?;
            elements.push(WithId {
                id: run.id,
                resource: resolved,
            });
        }
        let full_count = u64::try_from(count_row.count.unwrap_or_default()).unwrap_or_default();
        Ok(Paginated::new(elements, full_count, page.per_page(), page))
    }

    pub fn state(&self) -> RunState {
        if self.finished.is_some() {
            RunState::Finished
        } else if self.dequeued_at.is_some() {
            RunState::InProgress
        } else {
            RunState::Queued
        }
    }

    pub fn commit_url(&self) -> String {
        format!(
            "{repo_url}/commit/{commit}",
            repo_url = self.repository.resource.external_repo_url(),
            commit = self.commit,
        )
    }
}
