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
    esi::EsiMetadata,
    handlers::crypto,
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
