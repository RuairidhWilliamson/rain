mod filters;

use anyhow::Context as _;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use rain_ci_common::{
    db::{
        Db, Resource as _, WithId,
        repository::{Repository, RepositoryId, ResolvedRepository},
        run::{ResolvedRun, Run, RunId},
    },
    pagination::{Paginated, Pagination},
};

use crate::{AdminUser, AppError, AuthUser, User};

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

pub async fn repos(
    auth: AdminUser,
    Query(page): Query<Pagination>,
    State(db): State<Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "repos.html")]
    struct ReposPage {
        base: Base,
        paged_repos: Paginated<WithId<ResolvedRepository>>,
    }

    Ok(Html(
        ReposPage {
            base: Base::new(auth.user),
            paged_repos: ResolvedRepository::list(&db, &page)
                .await
                .context("list repos")?,
        }
        .render()?,
    ))
}

pub async fn repo(
    auth: AdminUser,
    Path(id): Path<RepositoryId>,
    Query(page): Query<Pagination>,
    State(db): State<Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "repo.html")]
    struct RepoPage {
        base: Base,
        repo_id: RepositoryId,
        repo: ResolvedRepository,
        paged_runs: Paginated<WithId<ResolvedRun>>,
    }
    Ok(Html(
        RepoPage {
            base: Base::new(auth.user),
            repo: Repository::get(&db, id)
                .await
                .context("get repo")?
                .resource
                .resolve(&db)
                .await?,
            repo_id: id,
            paged_runs: ResolvedRun::list_in_repo(&db, &page, id)
                .await
                .context("list repos")?,
        }
        .render()?,
    ))
}

pub async fn runs(
    auth: AdminUser,
    Query(page): Query<Pagination>,
    State(db): State<Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "runs.html")]
    struct RunsPage {
        base: Base,
        paged_runs: Paginated<WithId<ResolvedRun>>,
    }
    Ok(Html(
        RunsPage {
            base: Base::new(auth.user),
            paged_runs: ResolvedRun::list(&db, &page).await.context("list runs")?,
        }
        .render()?,
    ))
}

pub async fn run(
    auth: AdminUser,
    Path(id): Path<RunId>,
    State(db): State<Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "run.html")]
    struct RunPage {
        base: Base,
        run_id: RunId,
        run: ResolvedRun,
    }
    Ok(Html(
        RunPage {
            base: Base::new(auth.user),
            run: Run::get(&db, id).await?.resource.resolve(&db).await?,
            run_id: id,
        }
        .render()?,
    ))
}
