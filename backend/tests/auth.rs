// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Integration tests for the SSO auth flow hardening (harden-auth-flow §1, §4).
//
// §1.4 — /auth/callback rejects requests whose `auth_state` cookie is absent or
//         does not match the `state` query parameter, with HTTP 400, before any
//         in-flight record is consumed; the cookie is cleared on rejection.
// §4.1 — /auth/logout is POST-only; GET yields 405.
//
// The successful-callback path (token exchange + ESI public info + session
// cookie) is covered by the live HURL smoke test (§6.4): the callback handler
// calls hardcoded esi.evetech.net URLs that cannot be redirected to a mock at
// the integration-test layer, so a true happy-path test is not feasible here.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

use backend::{
    app_state::AppState,
    config::{Config, RateLimitConfig},
    esi::EsiMetadata,
    session::{InflightRecord, InflightStore, SessionStore},
};

const TEST_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Generous limits so the auth limiter never trips during these tests.
fn relaxed_rate_limit() -> RateLimitConfig {
    RateLimitConfig {
        esi_error_remain_threshold: 15,
        esi_bucket_remain_threshold: 10,
        api_per_millis: 1,
        api_burst: 1000,
        auth_per_millis: 1,
        auth_burst: 1000,
    }
}

fn build_state(pool: PgPool) -> AppState {
    AppState {
        config: Arc::new(Config {
            app_url: "http://localhost:3000".into(),
            encryption_secret: TEST_SECRET.into(),
            esi_client_id: "test_client_id".into(),
            esi_client_secret: "test_client_secret".into(),
            database_url: String::new(),
            rate_limit: relaxed_rate_limit(),
        }),
        db: pool.clone(),
        esi_metadata: Arc::new(EsiMetadata {
            authorization_endpoint: "https://login.eveonline.com/v2/oauth/authorize".into(),
            token_endpoint: "https://login.eveonline.com/v2/oauth/token".into(),
            jwks_uri: "https://login.eveonline.com/oauth/jwks".into(),
        }),
        session_store: SessionStore::new(pool),
        inflight_store: InflightStore::new(),
        http_client: reqwest::Client::new().into(),
    }
}

fn set_cookie_headers(resp: &axum::response::Response) -> Vec<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect()
}

// ── §1.4 callback state-cookie binding ───────────────────────────────────────

#[sqlx::test]
async fn callback_without_state_cookie_is_rejected(pool: PgPool) {
    let state = build_state(pool);
    // Seed a valid in-flight record so only the missing cookie can cause failure.
    state
        .inflight_store
        .add(InflightRecord {
            csrf_state: "valid-state".into(),
            return_to: None,
            account_id: None,
        })
        .await
        .unwrap();
    let router = backend::build_router(state.clone());

    let req = Request::builder()
        .method(Method::GET)
        .uri("/auth/callback?code=abc&state=valid-state")
        .header("x-forwarded-for", "10.20.0.1")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    // The in-flight record was NOT consumed (no token exchange attempted).
    assert!(state.inflight_store.take("valid-state").await.is_some());
}

#[sqlx::test]
async fn callback_with_mismatching_state_cookie_is_rejected(pool: PgPool) {
    let state = build_state(pool);
    state
        .inflight_store
        .add(InflightRecord {
            csrf_state: "valid-state".into(),
            return_to: None,
            account_id: None,
        })
        .await
        .unwrap();
    let router = backend::build_router(state.clone());

    let req = Request::builder()
        .method(Method::GET)
        .uri("/auth/callback?code=abc&state=valid-state")
        .header("x-forwarded-for", "10.20.0.2")
        .header(header::COOKIE, "auth_state=a-different-value")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    // The mismatch rejection clears the stale auth_state cookie.
    let cleared = set_cookie_headers(&resp)
        .into_iter()
        .any(|c| c.contains("auth_state=") && c.contains("Max-Age=0"));
    assert!(cleared, "rejection should clear the auth_state cookie");
    // The in-flight record was NOT consumed.
    assert!(state.inflight_store.take("valid-state").await.is_some());
}

// ── §4.1 logout is POST-only ─────────────────────────────────────────────────

#[sqlx::test]
async fn get_logout_yields_405(pool: PgPool) {
    let router = backend::build_router(build_state(pool));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/auth/logout")
        .header("x-forwarded-for", "10.20.0.3")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[sqlx::test]
async fn post_logout_without_session_redirects_home(pool: PgPool) {
    let router = backend::build_router(build_state(pool));

    let req = Request::builder()
        .method(Method::POST)
        .uri("/auth/logout")
        .header("x-forwarded-for", "10.20.0.4")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert!(resp.status().is_redirection());
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");
    // The session cookie is cleared with Secure set.
    let cleared = set_cookie_headers(&resp)
        .into_iter()
        .any(|c| c.contains("session=") && c.contains("Max-Age=0") && c.contains("Secure"));
    assert!(
        cleared,
        "logout should clear the session cookie with Secure"
    );
}
