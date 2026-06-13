// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Integration tests for the /api/v1/maps and /api/v1/acls endpoints.
//
// Exercise the full handler → service → db path against a real `#[sqlx::test]`
// database: ACL CRUD + members, map CRUD, attach/detach, the resolver path
// (a corporation grant lets a non-owner read a map; a deny refuses), slug
// conflict, and below-threshold 403.

use axum::{
    Router,
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

/// Inserts a character on `account_id` with the given corporation. Returns the
/// `eve_character.id` UUID. Used to drive the resolver matching path.
async fn insert_character(
    pool: &PgPool,
    account_id: Uuid,
    eve_character_id: i64,
    corporation_id: i64,
) -> Uuid {
    let row = sqlx::query!(
        r#"
        INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name)
        VALUES ($1, $2, $3, $4, 'Test Corp')
        RETURNING id
        "#,
        account_id,
        eve_character_id,
        format!("Char {eve_character_id}"),
        corporation_id,
    )
    .fetch_one(pool)
    .await
    .expect("insert character");
    row.id
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response body is valid JSON")
}

fn req(method: Method, uri: &str, cookie: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, cookie);
    let body = match body {
        Some(v) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).unwrap()
}

async fn send(router: &Router, request: Request<Body>) -> axum::response::Response {
    router.clone().oneshot(request).await.unwrap()
}

// ── ACL CRUD + members ─────────────────────────────────────────────────────────

#[sqlx::test]
async fn acl_crud_and_member_lifecycle(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    // Create
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &cookie,
            Some(json!({"name": "Corp ACL"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = json_body(resp).await;
    let acl_id = body["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["data"]["name"], "Corp ACL");

    // List shows it
    let resp = send(&router, req(Method::GET, "/api/v1/acls", &cookie, None)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    // Rename
    let resp = send(
        &router,
        req(
            Method::PATCH,
            &format!("/api/v1/acls/{acl_id}"),
            &cookie,
            Some(json!({"name": "Renamed ACL"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["data"]["name"], "Renamed ACL");

    // Add a corporation member
    let resp = send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/acls/{acl_id}/members"),
            &cookie,
            Some(json!({
                "member_type": "corporation",
                "eve_entity_id": 5000,
                "name": "Some Corp",
                "permission": "read",
            })),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let member_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // List members
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/acls/{acl_id}/members"),
            &cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["data"].as_array().unwrap().len(), 1);

    // Update member permission
    let resp = send(
        &router,
        req(
            Method::PATCH,
            &format!("/api/v1/acls/{acl_id}/members/{member_id}"),
            &cookie,
            Some(json!({"permission": "read_write"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["data"]["permission"], "read_write");

    // Remove member
    let resp = send(
        &router,
        req(
            Method::DELETE,
            &format!("/api/v1/acls/{acl_id}/members/{member_id}"),
            &cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Delete ACL
    let resp = send(
        &router,
        req(
            Method::DELETE,
            &format!("/api/v1/acls/{acl_id}"),
            &cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn corporation_member_cannot_be_granted_manage(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &cookie,
            Some(json!({"name": "A"})),
        ),
    )
    .await;
    let acl_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // manage on a corporation member is forbidden by the role-for-type rule.
    let resp = send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/acls/{acl_id}/members"),
            &cookie,
            Some(json!({
                "member_type": "corporation",
                "eve_entity_id": 5000,
                "permission": "manage",
            })),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn cannot_manage_acl_owned_by_another_account(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_owner, owner_cookie) = create_session(&state).await;
    let (_other, other_cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &owner_cookie,
            Some(json!({"name": "Mine"})),
        ),
    )
    .await;
    let acl_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // A different account cannot rename it (403, not 404).
    let resp = send(
        &router,
        req(
            Method::PATCH,
            &format!("/api/v1/acls/{acl_id}"),
            &other_cookie,
            Some(json!({"name": "Hijacked"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── Map CRUD + slug conflict ────────────────────────────────────────────────────

#[sqlx::test]
async fn map_crud_lifecycle(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    // Create
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &cookie,
            Some(json!({"name": "Chain", "slug": "chain-1"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = json_body(resp).await;
    let map_id = body["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["data"]["slug"], "chain-1");
    assert!(body["data"]["acls"].as_array().unwrap().is_empty());

    // Get
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/maps/{map_id}"),
            &cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Update
    let resp = send(
        &router,
        req(
            Method::PATCH,
            &format!("/api/v1/maps/{map_id}"),
            &cookie,
            Some(json!({"name": "Renamed", "slug": "chain-2"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["data"]["slug"], "chain-2");

    // List
    let resp = send(&router, req(Method::GET, "/api/v1/maps", &cookie, None)).await;
    assert_eq!(json_body(resp).await["data"].as_array().unwrap().len(), 1);

    // Delete (soft)
    let resp = send(
        &router,
        req(
            Method::DELETE,
            &format!("/api/v1/maps/{map_id}"),
            &cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Now gone from the list and 404 on get.
    let resp = send(&router, req(Method::GET, "/api/v1/maps", &cookie, None)).await;
    assert!(json_body(resp).await["data"].as_array().unwrap().is_empty());
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/maps/{map_id}"),
            &cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn duplicate_slug_returns_conflict(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    let body = json!({"name": "A", "slug": "taken"});
    let resp = send(
        &router,
        req(Method::POST, "/api/v1/maps", &cookie, Some(body.clone())),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = send(
        &router,
        req(Method::POST, "/api/v1/maps", &cookie, Some(body)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_body(resp).await["error"]["code"],
        "map_slug_already_exists"
    );
}

#[sqlx::test]
async fn invalid_slug_is_rejected(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &cookie,
            Some(json!({"name": "A", "slug": "Bad Slug"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── attach/detach + resolver path ───────────────────────────────────────────────

#[sqlx::test]
async fn corporation_grant_lets_non_owner_read_map(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_owner, owner_cookie) = create_session(&state).await;
    let (member_account, member_cookie) = create_session(&state).await;
    // The member's character is in corp 5000.
    insert_character(&pool, member_account, 9001, 5000).await;
    let router = backend::build_router(state);

    // Owner creates a map and an ACL granting corp 5000 read access, then attaches.
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &owner_cookie,
            Some(json!({"name": "Shared", "slug": "shared"})),
        ),
    )
    .await;
    let map_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &owner_cookie,
            Some(json!({"name": "Corp"})),
        ),
    )
    .await;
    let acl_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/acls/{acl_id}/members"),
            &owner_cookie,
            Some(
                json!({"member_type": "corporation", "eve_entity_id": 5000, "permission": "read"}),
            ),
        ),
    )
    .await;

    let resp = send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/maps/{map_id}/acls"),
            &owner_cookie,
            Some(json!({"acl_id": acl_id})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The member (non-owner) can now read the map and see it in their list.
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/maps/{map_id}"),
            &member_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = send(
        &router,
        req(Method::GET, "/api/v1/maps", &member_cookie, None),
    )
    .await;
    assert_eq!(json_body(resp).await["data"].as_array().unwrap().len(), 1);

    // But read-only is below manage — the member cannot update the map (403).
    let resp = send(
        &router,
        req(
            Method::PATCH,
            &format!("/api/v1/maps/{map_id}"),
            &member_cookie,
            Some(json!({"name": "Hijack", "slug": "hijack"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn deny_member_refuses_access(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_owner, owner_cookie) = create_session(&state).await;
    let (member_account, member_cookie) = create_session(&state).await;
    let member_char_id = insert_character(&pool, member_account, 9002, 6000).await;
    let router = backend::build_router(state);

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &owner_cookie,
            Some(json!({"name": "Closed", "slug": "closed"})),
        ),
    )
    .await;
    let map_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &owner_cookie,
            Some(json!({"name": "Mixed"})),
        ),
    )
    .await;
    let acl_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Grant read to the member's corp, then deny the member's character
    // directly: deny wins over the corp grant. Read and deny must sit on
    // *distinct* member identities — `acl_member_unique_entity` forbids a second
    // (acl, corporation, 6000) row, so the deny is applied to the character
    // instead (mirroring the `permissions::deny_overrides_all_grants` unit test).
    send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/acls/{acl_id}/members"),
            &owner_cookie,
            Some(
                json!({"member_type": "corporation", "eve_entity_id": 6000, "permission": "read"}),
            ),
        ),
    )
    .await;
    send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/acls/{acl_id}/members"),
            &owner_cookie,
            Some(json!({
                "member_type": "character",
                "eve_entity_id": 9002,
                "character_id": member_char_id,
                "permission": "deny"
            })),
        ),
    )
    .await;
    send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/maps/{map_id}/acls"),
            &owner_cookie,
            Some(json!({"acl_id": acl_id})),
        ),
    )
    .await;

    // Denied: the member cannot read.
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/maps/{map_id}"),
            &member_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn cannot_attach_acl_you_do_not_own(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_owner, owner_cookie) = create_session(&state).await;
    let (_other, other_cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    // Owner makes a map; the other account makes an ACL.
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &owner_cookie,
            Some(json!({"name": "M", "slug": "m"})),
        ),
    )
    .await;
    let map_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &other_cookie,
            Some(json!({"name": "Theirs"})),
        ),
    )
    .await;
    let foreign_acl = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Owner of the map cannot attach an ACL they don't own.
    let resp = send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/maps/{map_id}/acls"),
            &owner_cookie,
            Some(json!({"acl_id": foreign_acl})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── single-resource reads: GET /acls/{id} ───────────────────────────────────────

#[sqlx::test]
async fn get_acl_by_id_visibility(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_owner, owner_cookie) = create_session(&state).await;
    let (_other, other_cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    // Owner creates an ACL.
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &owner_cookie,
            Some(json!({"name": "Mine"})),
        ),
    )
    .await;
    let acl_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Owner reads it: 200.
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/acls/{acl_id}"),
            &owner_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["data"]["name"], "Mine");

    // Unrelated caller gets 404 (not 403) — existence is hidden.
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/acls/{acl_id}"),
            &other_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Unknown id is 404.
    let resp = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/acls/{}", Uuid::new_v4()),
            &owner_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── single-resource reads: GET /maps/by-slug/{slug} ─────────────────────────────

#[sqlx::test]
async fn get_map_by_slug_visibility(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_owner, owner_cookie) = create_session(&state).await;
    let (member_account, member_cookie) = create_session(&state).await;
    let (_stranger, stranger_cookie) = create_session(&state).await;
    insert_character(&pool, member_account, 9100, 7000).await;
    let router = backend::build_router(state);

    // Owner creates a map, an ACL granting corp 7000 read, attaches.
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &owner_cookie,
            Some(json!({"name": "Slugged", "slug": "slugged"})),
        ),
    )
    .await;
    let map_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &owner_cookie,
            Some(json!({"name": "Readers"})),
        ),
    )
    .await;
    let acl_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/acls/{acl_id}/members"),
            &owner_cookie,
            Some(
                json!({"member_type": "corporation", "eve_entity_id": 7000, "permission": "read"}),
            ),
        ),
    )
    .await;
    send(
        &router,
        req(
            Method::POST,
            &format!("/api/v1/maps/{map_id}/acls"),
            &owner_cookie,
            Some(json!({"acl_id": acl_id})),
        ),
    )
    .await;

    // Reader (corp grant) resolves it by slug: 200, and the owner sees the ACL
    // summary (the reader does not manage it, so their summary list is empty).
    let resp = send(
        &router,
        req(
            Method::GET,
            "/api/v1/maps/by-slug/slugged",
            &owner_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"]["id"], map_id);
    assert_eq!(body["data"]["acls"].as_array().unwrap().len(), 1);

    let resp = send(
        &router,
        req(
            Method::GET,
            "/api/v1/maps/by-slug/slugged",
            &member_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    // The reader cannot manage the ACL, so no summary is surfaced to them.
    assert!(
        json_body(resp).await["data"]["acls"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // A stranger with no permission gets 404.
    let resp = send(
        &router,
        req(
            Method::GET,
            "/api/v1/maps/by-slug/slugged",
            &stranger_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Unknown slug is 404.
    let resp = send(
        &router,
        req(
            Method::GET,
            "/api/v1/maps/by-slug/nope",
            &owner_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Soft-deleted slug is 404.
    send(
        &router,
        req(
            Method::DELETE,
            &format!("/api/v1/maps/{map_id}"),
            &owner_cookie,
            None,
        ),
    )
    .await;
    let resp = send(
        &router,
        req(
            Method::GET,
            "/api/v1/maps/by-slug/slugged",
            &owner_cookie,
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── atomic default-ACL creation ─────────────────────────────────────────────────

#[sqlx::test]
async fn default_acl_creation_seeds_main_and_attaches(pool: PgPool) {
    let state = build_state(pool.clone());
    let (account, cookie) = create_session(&state).await;
    // Give the account a main character.
    let main_char = sqlx::query!(
        r#"INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name, is_main)
           VALUES ($1, 9200, 'Boss', 8000, 'Test Corp', TRUE) RETURNING id"#,
        account,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let router = backend::build_router(state);

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &cookie,
            Some(json!({"name": "Home", "slug": "home", "default_acl": true})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Exactly one ACL exists, named after the map, owned by the caller.
    let acls = send(&router, req(Method::GET, "/api/v1/acls", &cookie, None)).await;
    let body = json_body(acls).await;
    let arr = body["data"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "Home");
    let acl_id = arr[0]["id"].as_str().unwrap().to_string();

    // The main is seeded as an admin character member.
    let members = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/acls/{acl_id}/members"),
            &cookie,
            None,
        ),
    )
    .await;
    let body = json_body(members).await;
    let arr = body["data"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["member_type"], "character");
    assert_eq!(arr[0]["permission"], "admin");
    assert_eq!(arr[0]["eve_entity_id"], 9200);
    assert_eq!(arr[0]["character_id"], main_char.id.to_string());

    // The ACL is attached to the map (visible in the map's summaries).
    let map = send(
        &router,
        req(Method::GET, "/api/v1/maps/by-slug/home", &cookie, None),
    )
    .await;
    let body = json_body(map).await;
    assert_eq!(body["data"]["acls"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["acls"][0]["id"], acl_id);
}

#[sqlx::test]
async fn default_acl_without_main_creates_empty_acl(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &cookie,
            Some(json!({"name": "Solo", "slug": "solo", "default_acl": true})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let acls = send(&router, req(Method::GET, "/api/v1/acls", &cookie, None)).await;
    let body = json_body(acls).await;
    let acl_id = body["data"][0]["id"].as_str().unwrap().to_string();

    let members = send(
        &router,
        req(
            Method::GET,
            &format!("/api/v1/acls/{acl_id}/members"),
            &cookie,
            None,
        ),
    )
    .await;
    assert!(
        json_body(members).await["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn default_acl_rolls_back_on_slug_conflict(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    // First map takes the slug.
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &cookie,
            Some(json!({"name": "First", "slug": "taken"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Second create with default_acl hits the slug conflict.
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &cookie,
            Some(json!({"name": "Second", "slug": "taken", "default_acl": true})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // No stray ACL was minted — the transaction rolled back.
    let acls = send(&router, req(Method::GET, "/api/v1/acls", &cookie, None)).await;
    assert!(json_body(acls).await["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn default_acl_and_acl_id_together_is_bad_request(pool: PgPool) {
    let state = build_state(pool.clone());
    let (_account, cookie) = create_session(&state).await;
    let router = backend::build_router(state);

    // An ACL to reference.
    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/acls",
            &cookie,
            Some(json!({"name": "X"})),
        ),
    )
    .await;
    let acl_id = json_body(resp).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = send(
        &router,
        req(
            Method::POST,
            "/api/v1/maps",
            &cookie,
            Some(json!({"name": "Both", "slug": "both", "acl_id": acl_id, "default_acl": true})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Nothing was created — the map does not exist.
    let maps = send(&router, req(Method::GET, "/api/v1/maps", &cookie, None)).await;
    assert!(json_body(maps).await["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn unauthenticated_requests_are_rejected(pool: PgPool) {
    let state = build_state(pool.clone());
    let router = backend::build_router(state);

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/maps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
