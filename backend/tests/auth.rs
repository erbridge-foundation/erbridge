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
    esi::test_support::{EsiClaims, jwks_cache_for, test_keypair},
    session::{InflightRecord, InflightStore, SessionStore},
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
            bind_addr: "0.0.0.0:3000".to_string(),
            rate_limit: relaxed_rate_limit(),
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

// ── harden-token-crypto §2.4 — callback rejects an unverifiable token ─────────

/// Like `build_state` but with the token endpoint pointed at `token_endpoint`
/// and a JWKS cache holding only `kid-good` (an unreachable refetch URI), so a
/// token signed by any other key cannot verify.
fn build_state_with_token_endpoint(pool: PgPool, token_endpoint: String) -> AppState {
    let mut state = build_state(pool);
    state.esi_metadata = Arc::new(EsiMetadata {
        authorization_endpoint: "https://login.eveonline.com/v2/oauth/authorize".into(),
        token_endpoint,
        jwks_uri: "http://127.0.0.1:0/unreachable".into(),
    });
    state.jwks = Arc::new(jwks_cache_for(&test_keypair("kid-good")));
    state
}

#[sqlx::test]
async fn callback_rejects_bad_signature_token_with_502_and_no_writes(pool: PgPool) {
    // The token endpoint hands back a JWT signed by a key absent from the JWKS
    // cache; verification must fail and the callback must 502 without writing
    // any account / character / token / session row.
    let signer = test_keypair("kid-evil");
    let token = signer.sign(&EsiClaims::valid(99, "Evil", "owner-x"));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": token,
            "refresh_token": "r",
            "expires_in": 1200,
        })))
        .mount(&server)
        .await;
    let token_endpoint = format!("{}/v2/oauth/token", server.uri());

    let state = build_state_with_token_endpoint(pool.clone(), token_endpoint);
    state
        .inflight_store
        .add(InflightRecord {
            csrf_state: "valid-state".into(),
            return_to: None,
            account_id: None,
        })
        .await
        .unwrap();
    let router = backend::build_router(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/auth/callback?code=abc&state=valid-state")
        .header("x-forwarded-for", "10.20.0.9")
        .header(header::COOKIE, "auth_state=valid-state")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);

    // No rows were written anywhere.
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM account")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(accounts, 0, "no account row should be written");
    let chars: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM eve_character")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(chars, 0, "no character/token row should be written");
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0, "no session row should be written");
}
