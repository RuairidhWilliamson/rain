mod filters;

use anyhow::Context as _;
use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
};

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

pub async fn runs(auth: AdminUser, State(db): State<db::Db>) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "runs.html")]
    struct RunsPage {
        user: User,
        runs: Vec<(rain_ci_common::RunId, rain_ci_common::Run)>,
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
    Path(id): Path<rain_ci_common::RunId>,
    State(db): State<db::Db>,
) -> Result<Html<String>, AppError> {
    #[derive(Template)]
    #[template(path = "run.html")]
    struct RunPage {
        user: User,
        run_id: rain_ci_common::RunId,
        run: rain_ci_common::Run,
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
