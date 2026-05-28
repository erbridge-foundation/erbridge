// Integration tests for the /api/v1/keys endpoints.
//
// These tests exercise the full handler → service → db path using a real
// test database provisioned by `#[sqlx::test]`. Authentication is exercised
// via both the session-cookie path (create) and the Bearer-token path
// (list, delete, 401-after-delete).

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

// Bring in the crate under test.  The integration test binary sees the public
// API of the `backend` crate, but we also need internal helpers for signing
// session JWTs and building AppState.  Because those items are `pub` within
// their own modules (the router is `pub fn build_router`), we can reach them
// through `backend::`.
use backend::{
    app_state::AppState,
    config::Config,
    esi::EsiMetadata,
    handlers::crypto,
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
        database_url: String::new(), // not used by the router during tests
    })
}

fn test_esi_metadata() -> Arc<EsiMetadata> {
    Arc::new(EsiMetadata {
        authorization_endpoint: "https://login.eveonline.com/v2/oauth/authorize".into(),
        token_endpoint: "https://login.eveonline.com/v2/oauth/token".into(),
        jwks_uri: "https://login.eveonline.com/oauth/jwks".into(),
    })
}

/// Builds a test `AppState` backed by the provided pool and a fresh session store.
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

/// Creates a real account row, inserts a session row, and returns
/// `(account_id, session_cookie_header_value)`.
async fn create_session(state: &AppState) -> (Uuid, String) {
    let account_id = backend::db::accounts::create_account(&state.db)
        .await
        .expect("create_account");

    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .expect("insert session");

    let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
    let jwt = crypto::sign_session_jwt(&session_id, &key_bytes).unwrap();
    let cookie_value = format!("session={jwt}");

    (account_id, cookie_value)
}

/// Reads the full response body and parses it as JSON.
async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response body is valid JSON")
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// POST /api/v1/keys with no credentials → 401.
#[sqlx::test(migrations = "./migrations")]
async fn create_key_unauthenticated_returns_401(pool: PgPool) {
    let state = build_state(pool);
    let app = backend::build_router(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"smoke","expires_at":null}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Full lifecycle: create via session cookie → list via Bearer → delete via Bearer
/// → subsequent request with same key returns 401.
#[sqlx::test(migrations = "./migrations")]
async fn api_key_lifecycle(pool: PgPool) {
    let state = build_state(pool);
    let app = backend::build_router(state.clone());

    let (_, cookie) = create_session(&state).await;

    // ── 1. Create key via session cookie ──────────────────────────────────────
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(r#"{"name":"smoke","expires_at":null}"#))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = json_body(resp).await;
    let plaintext = body["data"]["key"]
        .as_str()
        .expect("data.key must be a string")
        .to_owned();
    let key_id = body["data"]["id"]
        .as_str()
        .expect("data.id must be a string")
        .to_owned();

    // Key must match the erb_ prefix and the expected length.
    assert!(
        plaintext.starts_with("erb_"),
        "key must start with 'erb_', got: {plaintext}"
    );
    assert_eq!(
        plaintext.len(),
        47,
        "key must be 47 chars (4-char prefix + 43-char body)"
    );

    // Response must NOT contain the hash.
    assert!(
        body["data"].get("key_hash").is_none(),
        "response must not expose key_hash"
    );

    // ── 2. List keys via Bearer token ─────────────────────────────────────────
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/keys")
        .header(header::AUTHORIZATION, format!("Bearer {plaintext}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = json_body(resp).await;
    let keys = body["data"].as_array().expect("data must be an array");
    assert_eq!(keys.len(), 1, "exactly one key should be listed");
    assert_eq!(keys[0]["id"].as_str().unwrap(), key_id);
    assert_eq!(keys[0]["name"].as_str().unwrap(), "smoke");

    // List response must not include plaintext.
    assert!(
        keys[0].get("key").is_none(),
        "list response must not expose plaintext key"
    );

    // ── 3. Delete key via Bearer token ────────────────────────────────────────
    let req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/keys/{key_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {plaintext}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // ── 4. Subsequent request with deleted key → 401 ──────────────────────────
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/keys")
        .header(header::AUTHORIZATION, format!("Bearer {plaintext}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Delete a key that belongs to a different account → 404 (not 204).
#[sqlx::test(migrations = "./migrations")]
async fn delete_key_wrong_account_returns_404(pool: PgPool) {
    let state = build_state(pool);
    let app = backend::build_router(state.clone());

    // Account A creates a key.
    let (_, cookie_a) = create_session(&state).await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie_a)
        .body(Body::from(r#"{"name":"a-key","expires_at":null}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = json_body(resp).await;
    let key_id = body["data"]["id"].as_str().unwrap().to_owned();

    // Account B tries to delete account A's key using their own session cookie.
    let (_, cookie_b) = create_session(&state).await;
    let req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/keys/{key_id}"))
        .header(header::COOKIE, &cookie_b)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// POST /api/v1/keys with a name that already exists for the same account → 409.
#[sqlx::test(migrations = "./migrations")]
async fn create_key_duplicate_name_returns_409(pool: PgPool) {
    let state = build_state(pool);
    let app = backend::build_router(state.clone());

    let (_, cookie) = create_session(&state).await;

    // First create — succeeds.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(r#"{"name":"ci","expires_at":null}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Second create with the same name → 409 with api_key_name_already_exists code.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(r#"{"name":"ci","expires_at":null}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    let body = json_body(resp).await;
    assert_eq!(
        body["error"]["code"].as_str().unwrap(),
        "api_key_name_already_exists"
    );
}

/// Two accounts may each have an API key with the same name (the uniqueness is scoped per account).
#[sqlx::test(migrations = "./migrations")]
async fn create_key_same_name_different_accounts_returns_201(pool: PgPool) {
    let state = build_state(pool);
    let app = backend::build_router(state.clone());

    let (_, cookie_a) = create_session(&state).await;
    let (_, cookie_b) = create_session(&state).await;

    let make_req = |cookie: &str| {
        Request::builder()
            .method(Method::POST)
            .uri("/api/v1/keys")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, cookie.to_owned())
            .body(Body::from(r#"{"name":"shared","expires_at":null}"#))
            .unwrap()
    };

    let resp_a = app.clone().oneshot(make_req(&cookie_a)).await.unwrap();
    assert_eq!(resp_a.status(), StatusCode::CREATED);

    let resp_b = app.clone().oneshot(make_req(&cookie_b)).await.unwrap();
    assert_eq!(resp_b.status(), StatusCode::CREATED);
}

/// POST /api/v1/keys with an empty name → 400.
#[sqlx::test(migrations = "./migrations")]
async fn create_key_empty_name_returns_400(pool: PgPool) {
    let state = build_state(pool);
    let app = backend::build_router(state.clone());

    let (_, cookie) = create_session(&state).await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(r#"{"name":"   ","expires_at":null}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
