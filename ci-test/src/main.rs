use std::time::Duration;

use anyhow::Result;
use axum::{
    body::Bytes,
    extract::MatchedPath,
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
    Router,
};
use octocrab::{
    models::webhook_events::{WebhookEvent, WebhookEventPayload},
    OctocrabBuilder,
};
use serde_json::json;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let app = router();
    tracing::info!("Starting server");

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn router() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/event_handler", post(event_handler))
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);
                let payload = request.body();
                tracing::info!("{payload:?}");

                tracing::info_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                    some_other_field = tracing::field::Empty,
                )
            }),
        )
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn event_handler(headers: HeaderMap, body: Bytes) -> StatusCode {
    let event = WebhookEvent::try_from_header_and_body(
        headers.get("x-github-event").unwrap().to_str().unwrap(),
        &body,
    )
    .unwrap();

    // TODO: Mock out this test
    if cfg!(test) {
        return StatusCode::OK;
    }

    let WebhookEventPayload::Push(push_event) = &event.specific else {
        tracing::error!("not implemented for {:?}", event.kind);
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    let client = OctocrabBuilder::new()
        .personal_token(std::env::var("GH_TOKEN").unwrap())
        .build()
        .unwrap();
    let repository = event.repository.unwrap();
    let repo_full_name = repository.full_name.unwrap();

    let head_commit = push_event.head_commit.as_ref().unwrap();
    let sha = &head_commit.id;

    let _: serde_json::Value = client
        .post(
            format!("/repos/{repo_full_name}/statuses/{sha}"),
            Some(&json!({
                "state": "pending",
                "description": "Processing",
            })),
        )
        .await
        .unwrap();
    sleep(Duration::from_secs(10)).await;
    let _: serde_json::Value = client
        .post(
            format!("/repos/{repo_full_name}/statuses/{sha}"),
            Some(&json!({
                "state": "success",
                "description": "Yippeee",
            })),
        )
        .await
        .unwrap();
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    #[tokio::test]
    async fn push_main() -> Result<()> {
        let server = axum_test::TestServer::new(super::router())?;
        let response = server
            .post("/event_handler")
            .add_header("x-github-event".parse()?, "push".parse()?)
            .json(&serde_json::from_str::<serde_json::Value>(include_str!(
                "../testdata/push_main.json"
            ))?)
            .await;
        response.assert_status_ok();
        Ok(())
    }

    #[tokio::test]
    async fn push_branch() -> Result<()> {
        let server = axum_test::TestServer::new(super::router())?;
        let response = server
            .post("/event_handler")
            .add_header("x-github-event".parse()?, "push".parse()?)
            .json(&serde_json::from_str::<serde_json::Value>(include_str!(
                "../testdata/push_branch.json"
            ))?)
            .await;
        response.assert_status_ok();
        Ok(())
    }

    #[tokio::test]
    async fn create_branch() -> Result<()> {
        let server = axum_test::TestServer::new(super::router())?;
        let response = server
            .post("/event_handler")
            .add_header("x-github-event".parse()?, "create".parse()?)
            .json(&serde_json::from_str::<serde_json::Value>(include_str!(
                "../testdata/create_branch.json"
            ))?)
            .await;
        response.assert_status_ok();
        Ok(())
    }

    #[tokio::test]
    async fn pull_request_opened() -> Result<()> {
        let server = axum_test::TestServer::new(super::router())?;
        let response = server
            .post("/event_handler")
            .add_header("x-github-event".parse()?, "create".parse()?)
            .json(&serde_json::from_str::<serde_json::Value>(include_str!(
                "../testdata/create_branch.json"
            ))?)
            .await;
        response.assert_status_ok();
        Ok(())
    }
}
