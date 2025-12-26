mod filters;

use anyhow::Context as _;
use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
};
use rain_ci_common::{Repository, RepositoryId, Run, RunId};

use crate::{AdminUser, AppError, AuthUser, User, db};

pub async fn home(auth: Option<AuthUser>) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "landing.html")]
    struct PublicHomepage;

    #[derive(Template)]
    #[template(path = "home.html")]
    struct Homepage {
        user: User,
    }
    if let Some(auth) = auth {
        let homepage = Homepage { user: auth.user };
        Ok(Html(homepage.render()?))
    } else {
        Ok(Html(PublicHomepage.render()?))
    }
}

pub async fn profile(auth: AdminUser) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "profile.html")]
    struct Profile {
        user: User,
    }
    Ok(Html(Profile { user: auth.user }.render()?))
}

pub async fn repos(auth: AdminUser, State(db): State<db::Db>) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "repos.html")]
    struct ReposPage {
        user: User,
        repos: Vec<(RepositoryId, Repository)>,
    }
    Ok(Html(
        ReposPage {
            user: auth.user,
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
        user: User,
        repo_id: RepositoryId,
        repo: Repository,
    }
    Ok(Html(
        RepoPage {
            user: auth.user,
            repo: db.get_repo(&id).await.context("list repos")?,
            repo_id: id,
        }
        .render()?,
    ))
}

pub async fn runs(auth: AdminUser, State(db): State<db::Db>) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "runs.html")]
    struct RunsPage {
        user: User,
        runs: Vec<(RunId, Run)>,
    }
    Ok(Html(
        RunsPage {
            user: auth.user,
            runs: db.list_runs().await.context("list runs")?,
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
        user: User,
        run_id: RunId,
        run: Run,
    }
    Ok(Html(
        RunPage {
            user: auth.user,
            run: db.get_run(&id).await?,
            run_id: id,
        }
        .render()?,
    ))
}
