// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Integration tests for the GET /api/v1/me endpoint.
//
// These tests exercise the full handler → service → db path using a real
// test database. They verify that corporation_name and alliance_name come
// from the DB row (not ESI), and that token_status is derived from whether
// encrypted_refresh_token is non-NULL.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use chrono::{Duration, Utc};
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
    db::{accounts, characters},
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
        bind_addr: "0.0.0.0:3000".to_string(),
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
        jwks: std::sync::Arc::new(backend::esi::test_support::jwks_cache_for(
            &backend::esi::test_support::test_keypair("kid-1"),
        )),
        session_store: SessionStore::new(pool),
        inflight_store: InflightStore::new(),
        http_client: reqwest::Client::new().into(),
    }
}

async fn create_session(state: &AppState, account_id: Uuid) -> String {
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id)
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

fn test_key() -> Vec<u8> {
    vec![0u8; 32]
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// GET /api/v1/me with no credentials → 401.
#[sqlx::test(migrations = "./migrations")]
async fn get_me_unauthenticated_returns_401(pool: PgPool) {
    let state = build_state(pool);
    let app = backend::build_router(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/me")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body = json_body(resp).await;
    assert_eq!(body["error"]["code"], "unauthenticated");
    assert!(body.get("data").is_none());
}

/// GET /api/v1/me returns corporation_name and alliance_name from the DB row,
/// and token_status derived from whether encrypted_refresh_token is present.
///
/// Seeds three characters:
///   - char_active: has a refresh token → "active"
///   - char_expired: NULL refresh token → "expired"
///   - char_alliance: has a refresh token AND an alliance → checks alliance_name
#[sqlx::test(migrations = "./migrations")]
async fn get_me_returns_db_fields_and_token_status(pool: PgPool) {
    let state = build_state(pool.clone());
    let app = backend::build_router(state.clone());

    let account_id = accounts::create_account(&pool).await.unwrap();

    // Seed char_active: has a refresh token → "active"
    let mut tx = pool.begin().await.unwrap();
    let char_active_id = characters::upsert_tokens(
        &mut tx,
        account_id,
        10001,
        "Active Pilot",
        1000001,
        "Test Corporation",
        None,
        None,
        "client1",
        "access_tok",
        "refresh_tok",
        Utc::now() + Duration::hours(1),
        &["esi-location.read_location.v1".to_string()],
        "owner-hash",
        &test_key(),
    )
    .await
    .unwrap();
    characters::promote_if_no_main(&mut tx, account_id, char_active_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Seed char_expired: explicit token_status = 'token_expired' (no tokens).
    // Insert via raw SQL so we can set the status and NULL credentials directly.
    sqlx::query!(
        r#"
        INSERT INTO eve_character (
            account_id, eve_character_id, name,
            corporation_id, corporation_name, alliance_id, alliance_name,
            token_status
        ) VALUES ($1, 10002, 'Expired Pilot', 1000002, 'Ghost Corp', NULL, NULL,
            'token_expired')
        "#,
        account_id,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed char_alliance: has a refresh token AND an alliance name.
    let mut tx2 = pool.begin().await.unwrap();
    characters::upsert_tokens(
        &mut tx2,
        account_id,
        10003,
        "Alliance Pilot",
        1000003,
        "Alliance Corp",
        Some(500001),
        Some("Test Alliance"),
        "client1",
        "access_tok3",
        "refresh_tok3",
        Utc::now() + Duration::hours(1),
        &[],
        "owner-hash",
        &test_key(),
    )
    .await
    .unwrap();
    tx2.commit().await.unwrap();

    let cookie = create_session(&state, account_id).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/me")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = json_body(resp).await;
    let chars = body["data"]["characters"].as_array().unwrap();
    assert_eq!(chars.len(), 3);

    // Find each character by eve_character_id.
    let find = |esi_id: i64| {
        chars
            .iter()
            .find(|c| c["eve_character_id"].as_i64() == Some(esi_id))
            .unwrap()
    };

    let active = find(10001);
    assert_eq!(active["corporation_name"], "Test Corporation");
    assert!(active["alliance_name"].is_null());
    assert_eq!(active["token_status"], "active");

    let expired = find(10002);
    assert_eq!(expired["corporation_name"], "Ghost Corp");
    assert!(expired["alliance_name"].is_null());
    assert_eq!(expired["token_status"], "expired");

    let alliance = find(10003);
    assert_eq!(alliance["corporation_name"], "Alliance Corp");
    assert_eq!(alliance["alliance_name"], "Test Alliance");
    assert_eq!(alliance["token_status"], "active");

    // Sensitive fields must not appear.
    for c in chars {
        assert!(c.get("encrypted_access_token").is_none());
        assert!(c.get("encrypted_refresh_token").is_none());
        assert!(c.get("access_token_expires_at").is_none());
        assert!(c.get("esi_token_expires_at").is_none());
        assert!(c.get("esi_client_id").is_none());
    }
}
