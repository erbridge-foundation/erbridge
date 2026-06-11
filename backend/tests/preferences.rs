// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Integration tests for the /api/v1/me/preferences endpoints.
//
// Exercises the full handler → service → db path against a real test database:
// the authed GET default set, PATCH partial-merge semantics, validation errors
// mapping to 400, and the unauthenticated 401.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use backend::{
    app_state::AppState,
    config::Config,
    crypto,
    db::accounts,
    esi::EsiMetadata,
    session::{InflightStore, SessionStore},
};

// ── helpers ──────────────────────────────────────────────────────────────────

const TEST_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn test_config() -> Arc<Config> {
    Arc::new(Config {
        app_url: "http://localhost:3000".into(),
        encryption_secret: TEST_SECRET.into(),
        esi_client_id: "test_client_id".into(),
        esi_client_secret: "test_client_secret".into(),
        database_url: String::new(),
        rate_limit: Default::default(),
    })
}

fn test_esi_metadata() -> Arc<EsiMetadata> {
    Arc::new(EsiMetadata {
        authorization_endpoint: "https://login.eveonline.com/v2/oauth/authorize".into(),
        token_endpoint: "https://login.eveonline.com/v2/oauth/token".into(),
        jwks_uri: "https://login.eveonline.com/oauth/jwks".into(),
    })
}

fn build_state(pool: PgPool) -> AppState {
    AppState {
        config: test_config(),
        db: pool.clone(),
        esi_metadata: test_esi_metadata(),
        session_store: SessionStore::new(pool),
        inflight_store: InflightStore::new(),
        http_client: reqwest::Client::new().into(),
    }
}

async fn create_session(state: &AppState, account_id: Uuid) -> String {
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .expect("insert session");
    let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
    let jwt = crypto::sign_session_jwt(&session_id, &key_bytes).unwrap();
    format!("session={jwt}")
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response body is valid JSON")
}

fn patch_req(cookie: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/me/preferences")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn get_preferences_unauthenticated_returns_401(pool: PgPool) {
    let app = backend::build_router(build_state(pool));
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me/preferences")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = json_body(resp).await;
    assert_eq!(body["error"]["code"], "unauthenticated");
    assert!(body.get("data").is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn get_preferences_returns_defaults_for_new_account(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let cookie = create_session(&state, account_id).await;
    let app = backend::build_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me/preferences")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"]["text_size"], "auto");
    assert_eq!(body["data"]["reduce_motion"], "auto");
    assert_eq!(body["data"]["high_contrast"], "auto");
    assert_eq!(body["data"]["large_targets"], "off");
    assert_eq!(body["data"]["dyslexia_font"], "off");
    assert_eq!(body["data"]["locale"], "en");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_preferences_partial_merge_preserves_other_keys(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let cookie = create_session(&state, account_id).await;
    let app = backend::build_router(state);

    // First PATCH sets text_size.
    let resp = app
        .clone()
        .oneshot(patch_req(&cookie, json!({"text_size": "large"})))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"]["text_size"], "large");
    assert_eq!(body["data"]["reduce_motion"], "auto");

    // Second PATCH sets reduce_motion; text_size must survive.
    let resp = app
        .oneshot(patch_req(&cookie, json!({"reduce_motion": "on"})))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"]["text_size"], "large");
    assert_eq!(body["data"]["reduce_motion"], "on");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_preferences_sets_locale(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let cookie = create_session(&state, account_id).await;
    let app = backend::build_router(state);

    let resp = app
        .oneshot(patch_req(&cookie, json!({"locale": "en"})))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"]["locale"], "en");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_preferences_invalid_locale_returns_400(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let cookie = create_session(&state, account_id).await;
    let app = backend::build_router(state);

    let resp = app
        .oneshot(patch_req(&cookie, json!({"locale": "martian"})))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(body["error"]["code"], "bad_request");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_preferences_unknown_key_returns_400(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let cookie = create_session(&state, account_id).await;
    let app = backend::build_router(state);

    let resp = app
        .oneshot(patch_req(&cookie, json!({"not_a_pref": "x"})))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(body["error"]["code"], "bad_request");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_preferences_invalid_value_returns_400(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let cookie = create_session(&state, account_id).await;
    let app = backend::build_router(state);

    let resp = app
        .oneshot(patch_req(&cookie, json!({"text_size": "enormous"})))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(body["error"]["code"], "bad_request");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_preferences_empty_body_returns_400(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let cookie = create_session(&state, account_id).await;
    let app = backend::build_router(state);

    let resp = app.oneshot(patch_req(&cookie, json!({}))).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_preferences_unauthenticated_returns_401(pool: PgPool) {
    let app = backend::build_router(build_state(pool));
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/v1/me/preferences")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"text_size": "large"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
