// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Integration tests for Postgres-backed sessions.
//
// Covers the spec scenarios end-to-end against a real DB:
//   * session survives a backend restart (simulated by rebuilding AppState
//     over the same pool between requests),
//   * an expired row is rejected with 401,
//   * a successful cookie-authenticated request reissues the session cookie
//     with a fresh JWT,
//   * an API-key request does not touch the session row (last_seen_at
//     unchanged).

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use backend::{
    app_state::AppState,
    config::Config,
    crypto,
    esi::EsiMetadata,
    session::{InflightStore, SessionStore},
};

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

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response body is valid JSON")
}

fn sign_cookie(session_id: &str) -> String {
    let key = crypto::jwt_signing_key(TEST_SECRET).unwrap();
    let jwt = crypto::sign_session_jwt(session_id, &key).unwrap();
    format!("session={jwt}")
}

/// Returns the session JWT carried in a `Set-Cookie` header on the response,
/// if any.
fn refreshed_session_jwt(resp: &axum::response::Response) -> Option<String> {
    let raw = resp
        .headers()
        .get(header::SET_COOKIE)?
        .to_str()
        .ok()?
        .to_string();
    raw.split(';')
        .next()?
        .trim()
        .strip_prefix("session=")
        .map(|s| s.to_string())
}

// ── 1. Session survives restart ───────────────────────────────────────────────

#[sqlx::test]
async fn session_survives_backend_restart(pool: PgPool) {
    // First "boot" creates an account + session.
    let state1 = build_state(pool.clone());
    let account_id = backend::db::accounts::create_account(&state1.db)
        .await
        .unwrap();
    let session_id = Uuid::new_v4().to_string();
    state1
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .unwrap();
    let cookie = sign_cookie(&session_id);

    // Simulate a restart by dropping `state1` and rebuilding fresh state over
    // the same pool — no in-memory carryover from `state1` reaches `state2`.
    drop(state1);
    let state2 = build_state(pool.clone());
    let app = backend::build_router(state2);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(
        body["data"]["account"]["id"].as_str().unwrap(),
        account_id.to_string()
    );
}

// ── 2. Expired row is rejected with 401 ───────────────────────────────────────

#[sqlx::test]
async fn expired_session_is_rejected(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = backend::db::accounts::create_account(&state.db)
        .await
        .unwrap();
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .unwrap();

    // Force the row into the past.
    sqlx::query!(
        "UPDATE session SET expires_at = now() - interval '1 second' WHERE session_id = $1",
        session_id,
    )
    .execute(&pool)
    .await
    .unwrap();

    let cookie = sign_cookie(&session_id);
    let app = backend::build_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // No refreshed cookie on auth failure.
    assert!(refreshed_session_jwt(&resp).is_none());
}

// ── 3. Cookie is reissued on a successful authenticated request ───────────────

#[sqlx::test]
async fn cookie_is_reissued_on_success(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = backend::db::accounts::create_account(&state.db)
        .await
        .unwrap();
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .unwrap();
    let cookie = sign_cookie(&session_id);
    let app = backend::build_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let new_jwt = refreshed_session_jwt(&resp).expect("Set-Cookie present");
    // The freshly-minted JWT must verify back to the same session ID.
    let key = crypto::jwt_signing_key(TEST_SECRET).unwrap();
    let extracted = crypto::verify_session_jwt(&new_jwt, &key).unwrap();
    assert_eq!(extracted, session_id);
}

// ── 4. API-key request does not touch the session row ─────────────────────────

#[sqlx::test]
async fn api_key_request_does_not_touch_session(pool: PgPool) {
    let state = build_state(pool.clone());
    let app = backend::build_router(state.clone());

    // Create an account + session.
    let account_id = backend::db::accounts::create_account(&state.db)
        .await
        .unwrap();
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .unwrap();

    // Capture the session's last_seen_at and expires_at *before* the API-key
    // request. Force them into the past so any accidental refresh would be
    // observable.
    sqlx::query!(
        "UPDATE session
         SET last_seen_at = now() - interval '1 hour',
             expires_at   = now() + interval '6 days'
         WHERE session_id = $1",
        session_id,
    )
    .execute(&pool)
    .await
    .unwrap();
    let before = sqlx::query!(
        "SELECT last_seen_at, expires_at FROM session WHERE session_id = $1",
        session_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let before_seen: DateTime<Utc> = before.last_seen_at;
    let before_exp: DateTime<Utc> = before.expires_at;

    // Mint an API key for the same account via the service layer.
    let session_cookie = sign_cookie(&session_id);
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &session_cookie)
        .body(Body::from(r#"{"name":"sess-test","expires_at":null}"#))
        .unwrap();
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body = json_body(create_resp).await;
    let api_key = create_body["data"]["key"].as_str().unwrap().to_string();

    // Reset the row again (the cookie-auth POST above will have advanced it).
    sqlx::query!(
        "UPDATE session
         SET last_seen_at = $2,
             expires_at   = $3
         WHERE session_id = $1",
        session_id,
        before_seen,
        before_exp,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Issue an authenticated request with the API key.
    let api_resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::AUTHORIZATION, format!("Bearer {api_key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(api_resp.status(), StatusCode::OK);

    // The session row's timestamps must be unchanged.
    let after = sqlx::query!(
        "SELECT last_seen_at, expires_at FROM session WHERE session_id = $1",
        session_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after.last_seen_at, before_seen);
    assert_eq!(after.expires_at, before_exp);

    // And no refreshed session cookie should be on the API-key response.
    assert!(refreshed_session_jwt(&api_resp).is_none());
}
