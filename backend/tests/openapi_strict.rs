// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// OpenAPI strict-drift and doc-coverage tests.
//
// 2d.6 — strict-drift: every documented route's actual response validates against
//         its declared OpenAPI schema.
// 2d.7 — doc-coverage: every route in `registered_api_v1_routes()` is documented.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use jsonschema::JSONSchema;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{collections::HashSet, sync::Arc};
use tower::ServiceExt;
use utoipa::OpenApi;
use uuid::Uuid;

use backend::{
    app_state::AppState,
    config::Config,
    crypto,
    esi::EsiMetadata,
    openapi::ApiDoc,
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
        catalog: Default::default(),
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

async fn create_session(state: &AppState) -> (Uuid, String) {
    let account_id = backend::db::accounts::create_account(&state.db)
        .await
        .expect("create_account");

    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id)
        .await
        .expect("insert session");

    let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
    let jwt = crypto::sign_session_jwt(&session_id, &key_bytes).unwrap();
    (account_id, format!("session={jwt}"))
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response body is valid JSON")
}

fn openapi_doc_json() -> Value {
    serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI doc serialises")
}

/// Compile the JSON Schema for `(path, method, status)` from the OpenAPI doc.
///
/// Wraps the schema in a synthetic document that re-exports all OpenAPI
/// component schemas under `$defs` so that `$ref: "#/components/schemas/Foo"`
/// resolves correctly regardless of jsonschema draft version.
fn compile_schema(doc_json: &Value, path: &str, method: &str, status: &str) -> JSONSchema {
    let pointer = format!(
        "/paths/{}/{}/responses/{}/content/application~1json/schema",
        path.replace('/', "~1"),
        method,
        status
    );
    let response_schema = doc_json
        .pointer(&pointer)
        .unwrap_or_else(|| panic!("schema not found in OpenAPI doc at {pointer}"))
        .clone();

    // Build a wrapper document that has the same structure as the OpenAPI doc
    // so that `$ref: "#/components/schemas/Foo"` resolves against it.
    let wrapper = json!({
        "components": doc_json.pointer("/components").cloned().unwrap_or(json!({})),
        "allOf": [response_schema],
    });

    JSONSchema::compile(&wrapper)
        .unwrap_or_else(|e| panic!("failed to compile schema at {pointer}: {e}"))
}

fn assert_schema(doc_json: &Value, path: &str, method: &str, status: &str, body: &Value) {
    let compiled = compile_schema(doc_json, path, method, status);
    let errors: Vec<_> = compiled
        .validate(body)
        .err()
        .into_iter()
        .flatten()
        .collect();
    assert!(
        errors.is_empty(),
        "OpenAPI drift on {method} {path} -> {status}: {errors:?}\nbody: {body}"
    );
}

// ── 2d.7 doc-coverage ─────────────────────────────────────────────────────────

#[test]
fn all_registered_routes_are_documented() {
    let doc_json = openapi_doc_json();

    let documented: HashSet<(String, String)> = doc_json
        .pointer("/paths")
        .expect("/paths present")
        .as_object()
        .expect("/paths is object")
        .iter()
        .flat_map(|(path, methods)| {
            methods
                .as_object()
                .expect("methods is object")
                .keys()
                .map(move |m| (path.clone(), m.clone()))
                .collect::<Vec<_>>()
        })
        .collect();

    for r in backend::registered_api_v1_routes() {
        assert!(
            documented.contains(&r),
            "route {:?} is registered in the router but missing from the OpenAPI document",
            r
        );
    }
}

// ── fail-closed auth-coverage ──────────────────────────────────────────────────
//
// Auth is opt-in per handler (the AuthenticatedAccount extractor), so a new
// /api/v1 handler that forgets it is silently public. This test makes the
// versioned surface fail-closed: every registered /api/v1 route MUST declare a
// non-empty `security` requirement in the OpenAPI document. /api/health is
// intentionally public and is NOT in registered_api_v1_routes(), so it is
// correctly out of scope.

#[test]
fn all_registered_v1_routes_declare_auth() {
    let doc_json = openapi_doc_json();

    for (path, method) in backend::registered_api_v1_routes() {
        let pointer = format!("/paths/{}/{}/security", path.replace('/', "~1"), method);
        let security = doc_json.pointer(&pointer);

        let declares_auth = security
            .and_then(|v| v.as_array())
            .is_some_and(|arr| !arr.is_empty());

        assert!(
            declares_auth,
            "registered v1 route {method} {path} declares no `security` requirement \
             in the OpenAPI document — it may be accidentally public (missing the \
             AuthenticatedAccount extractor / security(...) annotation)"
        );
    }
}

// ── fail-closed admin-coverage ─────────────────────────────────────────────────
//
// Admin gating is opt-in per handler (the AdminAccount extractor), so a new
// /api/v1/admin/* handler that forgets it is silently NOT admin-gated. This
// test makes the admin surface fail-closed *behaviourally*: every route in
// registered_admin_routes() MUST reject an unauthenticated caller (401) and a
// non-admin session (403). A handler missing the extractor would answer
// differently (e.g. 200/404/405) and fail here. Mirrors
// `all_registered_v1_routes_declare_auth` for the admin tier.

/// Substitutes placeholder values for known path params so a registered route
/// template becomes a concrete request URI.
fn concrete_admin_uri(path: &str) -> String {
    path.replace("{id}", &Uuid::new_v4().to_string())
        .replace("{eve_character_id}", "12345")
}

fn method_from(method: &str) -> Method {
    Method::from_bytes(method.to_uppercase().as_bytes()).expect("valid method")
}

#[sqlx::test]
async fn admin_routes_reject_unauthenticated_401(pool: PgPool) {
    let state = build_state(pool);
    let router = backend::build_router(state);

    for (path, method) in backend::registered_admin_routes() {
        let uri = concrete_admin_uri(&path);
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method_from(&method))
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "admin route {method} {path} must reject an unauthenticated caller with 401 \
             (is it missing the AdminAccount extractor?)"
        );
    }
}

#[sqlx::test]
async fn admin_routes_reject_non_admin_403(pool: PgPool) {
    let state = build_state(pool);
    // A plain session (create_session makes a non-admin account).
    let (_account_id, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    for (path, method) in backend::registered_admin_routes() {
        let uri = concrete_admin_uri(&path);
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method_from(&method))
                    .uri(&uri)
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "admin route {method} {path} must reject a non-admin session with 403 \
             (is it missing the AdminAccount extractor?)"
        );
    }
}

// ── 2d.6 strict-drift ─────────────────────────────────────────────────────────

#[sqlx::test]
async fn get_me_200_matches_schema(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
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
    assert_schema(&doc, "/api/v1/me", "get", "200", &json_body(resp).await);
}

#[sqlx::test]
async fn get_me_401_without_session(pool: PgPool) {
    let doc = openapi_doc_json();

    let resp = backend::build_router(build_state(pool))
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_schema(&doc, "/api/v1/me", "get", "401", &json_body(resp).await);
}

#[sqlx::test]
async fn get_preferences_200_matches_schema(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
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
    assert_schema(
        &doc,
        "/api/v1/me/preferences",
        "get",
        "200",
        &json_body(resp).await,
    );
}

#[sqlx::test]
async fn patch_preferences_200_matches_schema(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/v1/me/preferences")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"text_size": "large"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_schema(
        &doc,
        "/api/v1/me/preferences",
        "patch",
        "200",
        &json_body(resp).await,
    );
}

#[sqlx::test]
async fn patch_preferences_400_on_unknown_key(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/v1/me/preferences")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"bogus_key": "x"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_schema(
        &doc,
        "/api/v1/me/preferences",
        "patch",
        "400",
        &json_body(resp).await,
    );
}

#[sqlx::test]
async fn post_keys_201_matches_schema(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/keys")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"name": "test", "expires_at": null}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert_schema(&doc, "/api/v1/keys", "post", "201", &json_body(resp).await);
}

#[sqlx::test]
async fn get_keys_200_matches_schema(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/keys")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_schema(&doc, "/api/v1/keys", "get", "200", &json_body(resp).await);
}

#[sqlx::test]
async fn delete_key_404_when_not_found(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();
    let missing_id = Uuid::new_v4();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/v1/keys/{missing_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_schema(
        &doc,
        "/api/v1/keys/{id}",
        "delete",
        "404",
        &json_body(resp).await,
    );
}

#[sqlx::test]
async fn delete_account_204(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/v1/account")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 204 No Content — no body to validate against the schema.
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn delete_account_clears_session_cookie_and_does_not_refresh(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/v1/account")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let set_cookies: Vec<String> = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();

    // Exactly one Set-Cookie header — proves the refresh middleware did not
    // append a competing refreshed session cookie alongside the cleared one.
    assert_eq!(
        set_cookies.len(),
        1,
        "expected exactly one Set-Cookie, got {set_cookies:?}"
    );

    let header_value = &set_cookies[0];
    assert!(
        header_value.contains("session="),
        "Set-Cookie should target the session cookie, got {header_value:?}"
    );
    assert!(
        header_value.contains("Max-Age=0"),
        "Set-Cookie should clear the session, got {header_value:?}"
    );
}

#[sqlx::test]
async fn entities_search_401_without_session(pool: PgPool) {
    let doc = openapi_doc_json();

    let resp = backend::build_router(build_state(pool))
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/entities/search?q=wasp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_schema(
        &doc,
        "/api/v1/entities/search",
        "get",
        "401",
        &json_body(resp).await,
    );
}

#[sqlx::test]
async fn entities_search_400_on_short_fragment(pool: PgPool) {
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/entities/search?q=wa")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_schema(
        &doc,
        "/api/v1/entities/search",
        "get",
        "400",
        &json_body(resp).await,
    );
}

#[sqlx::test]
async fn entities_search_200_unavailable_when_no_token(pool: PgPool) {
    // A fresh account has no character with a usable token, so the search is
    // hermetically "unavailable" (no live ESI needed) — a 200 with empty groups.
    let state = build_state(pool);
    let (_account_id, cookie) = create_session(&state).await;
    let doc = openapi_doc_json();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/entities/search?q=wasp")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"]["unavailable"], serde_json::json!(true));
    assert_schema(&doc, "/api/v1/entities/search", "get", "200", &body);
}

#[sqlx::test]
async fn get_health_200_matches_schema(pool: PgPool) {
    let doc = openapi_doc_json();

    // Public route — no session cookie.
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
    assert_schema(&doc, "/api/health", "get", "200", &json_body(resp).await);
}
