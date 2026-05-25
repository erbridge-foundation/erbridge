// Integration test for the GET /api/health endpoint.
//
// Exercises the full handler → service → db path against a real test database.
// Verifies the flat (unenveloped) response shape, a healthy db component, and
// that version/commit are populated.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

use backend::{
    app_state::AppState,
    config::Config,
    esi::EsiMetadata,
    session::{InflightStore, SessionStore},
};

const TEST_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn build_state(pool: PgPool) -> AppState {
    AppState {
        config: Arc::new(Config {
            app_url: "http://localhost:3000".into(),
            encryption_secret: TEST_SECRET.into(),
            esi_client_id: "test_client_id".into(),
            esi_client_secret: "test_client_secret".into(),
            database_url: String::new(),
        }),
        db: pool.clone(),
        esi_metadata: Arc::new(EsiMetadata {
            authorization_endpoint: "https://login.eveonline.com/v2/oauth/authorize".into(),
            token_endpoint: "https://login.eveonline.com/v2/oauth/token".into(),
            jwks_uri: "https://login.eveonline.com/oauth/jwks".into(),
        }),
        session_store: SessionStore::new(pool.clone()),
        inflight_store: InflightStore::new(),
        http_client: reqwest::Client::new().into(),
    }
}

#[sqlx::test]
async fn get_health_returns_ok_snapshot(pool: PgPool) {
    let resp = backend::build_router(build_state(pool))
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();

    // Flat shape, not enveloped.
    assert!(body.get("data").is_none(), "response must not be enveloped");

    assert_eq!(body["status"], "ok");
    assert!(
        body["version"].as_str().is_some_and(|s| !s.is_empty()),
        "version must be a non-empty string"
    );
    assert!(
        body["commit"].as_str().is_some_and(|s| !s.is_empty()),
        "commit must be a non-empty string"
    );

    let components = body["components"].as_array().unwrap();
    assert_eq!(components.len(), 1);
    assert_eq!(components[0]["name"], "db");
    assert_eq!(components[0]["status"], "ok");
}
