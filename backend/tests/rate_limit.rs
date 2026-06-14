// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Integration tests for the inbound per-IP rate limiters (add-esi-rate-limit-backoff §6).
//
// §6.1 — /api/* past the limit → 429 with the standard envelope,
//         error.code = "rate_limited", and a Retry-After header; under-limit
//         requests are unaffected.
// §6.2 — /auth/* past the limit → 302 redirect to the too-busy page (NOT a JSON
//         envelope), and the handler is not invoked.
// §6.3 — per-IP isolation: distinct source keys (X-Forwarded-For) throttle
//         independently.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

use backend::{
    AUTH_TOO_BUSY_PATH,
    app_state::AppState,
    config::{Config, RateLimitConfig},
    esi::EsiMetadata,
    session::{InflightStore, SessionStore},
};

const TEST_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Tiny bursts so the limiter trips in a handful of requests, keeping the tests
/// fast and deterministic. A long replenishment window means a single test's
/// burst is not refilled mid-run.
fn tight_rate_limit() -> RateLimitConfig {
    RateLimitConfig {
        esi_error_remain_threshold: 15,
        esi_bucket_remain_threshold: 10,
        api_per_millis: 60_000, // ~1 token / minute → effectively burst-only
        api_burst: 3,
        auth_per_millis: 60_000,
        auth_burst: 2,
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
            bind_addr: "0.0.0.0:3000".to_string(),
            rate_limit: tight_rate_limit(),
            catalog: Default::default(),
        }),
        db: pool.clone(),
        esi_metadata: Arc::new(EsiMetadata {
            authorization_endpoint: "https://login.eveonline.com/v2/oauth/authorize".into(),
            token_endpoint: "https://login.eveonline.com/v2/oauth/token".into(),
            jwks_uri: "https://login.eveonline.com/oauth/jwks".into(),
        }),
        jwks: std::sync::Arc::new(backend::esi::test_support::jwks_cache_for(
            &backend::esi::test_support::test_keypair("kid-1"),
        )),
        session_store: SessionStore::new(pool),
        inflight_store: InflightStore::new(),
        http_client: reqwest::Client::new().into(),
    }
}

/// Builds a GET request for `uri` attributed to source `ip` via X-Forwarded-For
/// so the limiter keys per simulated client.
fn get_from(uri: &str, ip: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap()
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ── §6.1 /api/* throttling ──────────────────────────────────────────────────────

#[sqlx::test]
async fn api_under_limit_is_unaffected(pool: PgPool) {
    let router = backend::build_router(build_state(pool));

    // burst = 3; the first three requests are within budget. Unauthenticated, so
    // each is a normal 401 (NOT a 429) — the limiter let them reach the handler.
    for _ in 0..3 {
        let resp = router
            .clone()
            .oneshot(get_from("/api/v1/me", "10.0.0.1"))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "within-budget /api request should reach the handler (401), not be throttled"
        );
    }
}

#[sqlx::test]
async fn api_over_limit_returns_rate_limited_envelope_with_retry_after(pool: PgPool) {
    let router = backend::build_router(build_state(pool));

    // Exhaust the burst (3), then the next request is throttled.
    let mut throttled = None;
    for _ in 0..10 {
        let resp = router
            .clone()
            .oneshot(get_from("/api/v1/me", "10.0.0.2"))
            .await
            .unwrap();
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = Some(resp);
            break;
        }
    }

    let resp = throttled.expect("expected a 429 after exhausting the burst");

    // Retry-After header present.
    assert!(
        resp.headers().get(header::RETRY_AFTER).is_some(),
        "throttled /api response must carry a Retry-After header"
    );

    // Standard envelope with the canonical code.
    let body = json_body(resp).await;
    assert_eq!(body["error"]["code"], "rate_limited");
    assert!(
        body["error"]["message"].as_str().is_some(),
        "envelope must carry a human-readable message"
    );
}

// ── §6.2 /auth/* throttling redirects ────────────────────────────────────────────

#[sqlx::test]
async fn auth_over_limit_redirects_to_too_busy_not_envelope(pool: PgPool) {
    let router = backend::build_router(build_state(pool));

    // auth burst = 2; hammer /auth/callback (the expensive endpoint).
    let mut throttled = None;
    for _ in 0..10 {
        let resp = router
            .clone()
            .oneshot(get_from("/auth/callback?code=x&state=y", "10.0.0.3"))
            .await
            .unwrap();
        if resp.status() == StatusCode::SEE_OTHER
            || resp.status() == StatusCode::TEMPORARY_REDIRECT
            || resp.status().is_redirection()
        {
            // Could be the handler's own redirect (invalid state) OR the limiter.
            // We distinguish by the Location once throttled enough.
            let location = resp
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if location == AUTH_TOO_BUSY_PATH {
                throttled = Some(resp);
                break;
            }
        }
    }

    let resp = throttled
        .expect("expected a redirect to the too-busy page after exhausting the auth burst");
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert_eq!(location, AUTH_TOO_BUSY_PATH);

    // It must NOT be a JSON rate_limited envelope. A redirect carries an empty
    // body, so parsing it as the error envelope must not yield an `error` field.
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: Option<Value> = serde_json::from_slice(&bytes).ok();
    assert!(
        parsed.and_then(|v| v.get("error").cloned()).is_none(),
        "throttled /auth response must not carry the JSON error envelope"
    );
}

// ── §6.3 per-IP isolation ────────────────────────────────────────────────────────

#[sqlx::test]
async fn api_limit_is_per_ip(pool: PgPool) {
    let router = backend::build_router(build_state(pool));

    // Client A exhausts its burst and gets throttled.
    let mut a_throttled = false;
    for _ in 0..10 {
        let resp = router
            .clone()
            .oneshot(get_from("/api/v1/me", "10.1.0.1"))
            .await
            .unwrap();
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            a_throttled = true;
            break;
        }
    }
    assert!(a_throttled, "client A should be throttled after its burst");

    // Client B (different IP) is unaffected — its first request still reaches the
    // handler (401), proving the buckets are independent.
    let resp_b = router
        .clone()
        .oneshot(get_from("/api/v1/me", "10.1.0.2"))
        .await
        .unwrap();
    assert_eq!(
        resp_b.status(),
        StatusCode::UNAUTHORIZED,
        "a different client IP must not be throttled by client A's exhaustion"
    );
}
