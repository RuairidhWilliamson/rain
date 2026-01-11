mod filters;

use std::num::NonZero;

use anyhow::Context as _;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use rain_ci_common::{Repository, RepositoryId, Run, RunId};

use crate::{
    AdminUser, AppError, AuthUser, User,
    db::{self, Paginated},
};

struct Base {
    user: User,
    rain_version: &'static str,
}

impl Base {
    fn new(user: User) -> Self {
        Self {
            user,
            rain_version: env!("CARGO_PKG_VERSION"),
        }
    }
}

pub async fn home(auth: Option<AuthUser>) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "landing.html")]
    struct PublicHomepage;

    #[derive(Template)]
    #[template(path = "home.html")]
    struct Homepage {
        base: Base,
    }
    if let Some(auth) = auth {
        let homepage = Homepage {
            base: Base::new(auth.user),
        };
        Ok(Html(homepage.render()?))
    } else {
        Ok(Html(PublicHomepage.render()?))
    }
}

pub async fn profile(auth: AdminUser) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "profile.html")]
    struct Profile {
        base: Base,
    }
    Ok(Html(
        Profile {
            base: Base::new(auth.user),
        }
        .render()?,
    ))
}

pub async fn repos(auth: AdminUser, State(db): State<db::Db>) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "repos.html")]
    struct ReposPage {
        base: Base,
        repos: Vec<(RepositoryId, Repository)>,
    }
    Ok(Html(
        ReposPage {
            base: Base::new(auth.user),
            repos: db.list_repos().await.context("list repos")?,
        }
        .render()?,
    ))
}

pub async fn repo(
    auth: AdminUser,
    Path(id): Path<RepositoryId>,
    State(db): State<db::Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "repo.html")]
    struct RepoPage {
        base: Base,
        repo_id: RepositoryId,
        repo: Repository,
        runs: Vec<(RunId, Run)>,
    }
    Ok(Html(
        RepoPage {
            base: Base::new(auth.user),
            repo: db.get_repo(&id).await.context("list repos")?,
            repo_id: id,
            runs: db.list_runs_in_repo(&id).await.context("list runs")?,
        }
        .render()?,
    ))
}

pub async fn runs(
    auth: AdminUser,
    Query(page): Query<Pagination>,
    State(db): State<db::Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "runs.html")]
    struct RunsPage {
        base: Base,
        paged_runs: Paginated<(RunId, Run)>,
    }
    Ok(Html(
        RunsPage {
            base: Base::new(auth.user),
            paged_runs: db.list_runs(&page).await.context("list runs")?,
        }
        .render()?,
    ))
}

pub async fn run(
    auth: AdminUser,
    Path(id): Path<RunId>,
    State(db): State<db::Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "run.html")]
    struct RunPage {
        base: Base,
        run_id: RunId,
        run: Run,
    }
    Ok(Html(
        RunPage {
            base: Base::new(auth.user),
            run: db.get_run(&id).await?,
            run_id: id,
        }
        .render()?,
    ))
}

#[derive(serde::Deserialize)]
pub struct Pagination {
    // The page number starting at 1
    pub page: Option<NonZero<u64>>,
}

impl Pagination {
    pub fn page_numberz(&self) -> anyhow::Result<i64> {
        match self.page {
            Some(x) => Ok(i64::try_from(x.get())? - 1),
            None => Ok(0),
        }
    }
}
