// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Integration tests for the /api/v1/admin/* handlers (section 7). Drives the
// real router with an admin session cookie and asserts the HTTP contract for
// every endpoint, plus the key error paths (last-admin 409, self-block 409,
// unblock 404) and the non-admin/unauthenticated rejections.
//
// The block handler fetches an ESI snapshot best-effort; the test http_client
// uses a short timeout so that fetch fails fast and returns (None, None) — the
// block still succeeds (enforcement keys on the id), keeping these tests
// hermetic and quick. Block teardown semantics are covered at the service layer
// (services::admin tests).

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use backend::{
    app_state::AppState,
    config::Config,
    crypto,
    db::{accounts, blocks},
    esi::EsiMetadata,
    session::{InflightStore, SessionStore},
};

const TEST_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn build_state(pool: PgPool) -> AppState {
    AppState {
        config: Arc::new(Config {
            app_url: "http://localhost:3000".into(),
            esi_callback_url: "http://localhost:3000/auth/callback".into(),
            encryption_secret: TEST_SECRET.into(),
            esi_client_id: "test_client_id".into(),
            esi_client_secret: "test_client_secret".into(),
            database_url: String::new(),
            bind_addr: "0.0.0.0:3000".to_string(),
            rate_limit: Default::default(),
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
        // Short timeout: the block handler's best-effort ESI snapshot fetch
        // fails fast (-> None/None) rather than reaching out to real ESI.
        http_client: reqwest::Client::builder()
            .timeout(Duration::from_millis(1))
            .build()
            .unwrap()
            .into(),
    }
}

/// Creates an account (optionally admin) with a session, returning
/// `(account_id, cookie)`.
async fn session_for(state: &AppState, admin: bool) -> (Uuid, String) {
    let account_id = accounts::create_account(&state.db).await.unwrap();
    if admin {
        let mut tx = state.db.begin().await.unwrap();
        accounts::set_server_admin(&mut tx, account_id, true)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id)
        .await
        .unwrap();
    let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
    let jwt = crypto::sign_session_jwt(&session_id, &key_bytes).unwrap();
    (account_id, format!("session={jwt}"))
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn req(method: Method, uri: &str, cookie: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    match body {
        Some(v) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

// ── accounts list ──────────────────────────────────────────────────────────────

#[sqlx::test]
async fn list_accounts_returns_accounts_with_characters(pool: PgPool) {
    let state = build_state(pool);
    let (admin_id, cookie) = session_for(&state, true).await;
    // Give the admin a character so the per-account characters array is exercised.
    sqlx::query!(
        "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name, is_main)
         VALUES ($1, 9001, 'Admin Main', 1, 'Corp', TRUE)",
        admin_id
    )
    .execute(&state.db)
    .await
    .unwrap();

    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/accounts",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    let accounts = body["data"].as_array().unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0]["is_server_admin"], true);
    assert_eq!(accounts[0]["characters"][0]["eve_character_id"], 9001);
    assert_eq!(accounts[0]["characters"][0]["name"], "Admin Main");
}

// ── character search ─────────────────────────────────────────────────────────

#[sqlx::test]
async fn search_characters_resolves_to_owning_account(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;
    let target = accounts::create_account(&state.db).await.unwrap();
    sqlx::query!(
        "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name)
         VALUES ($1, 7777, 'Pilgrim', 1, 'Corp')",
        target
    )
    .execute(&state.db)
    .await
    .unwrap();

    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/characters/search?q=pil",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    let results = body["data"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["eve_character_id"], 7777);
    assert_eq!(results[0]["account_id"], target.to_string());
    // Enriched fields (block-search picker reuses this shape).
    assert_eq!(results[0]["already_blocked"], false);
    assert!(
        results[0]["portrait_url"]
            .as_str()
            .unwrap()
            .contains("/characters/7777/portrait")
    );
}

// ── ESI character search ───────────────────────────────────────────────────────

#[sqlx::test]
async fn esi_search_rejects_short_fragment_400(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;

    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/characters/esi-search?q=wa",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(body["error"]["code"], "bad_request");
}

#[sqlx::test]
async fn esi_search_degrades_gracefully_when_esi_unreachable(pool: PgPool) {
    // build_state uses a 1ms-timeout http client, so the admin's token refresh
    // / ESI search fails fast → the endpoint returns 200 with unavailable=true,
    // never a 5xx. The admin needs a main character with token material for the
    // token path to be attempted; session_for(admin) creates the account, so add
    // a main with an (expired) token to drive the refresh→unreachable path.
    let state = build_state(pool);
    let (admin_id, cookie) = session_for(&state, true).await;
    sqlx::query!(
        "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name, is_main, encrypted_refresh_token, access_token_expires_at)
         VALUES ($1, 4242, 'Admin Main', 1, 'Corp', TRUE, '\\x00'::bytea, now() - interval '1 hour')",
        admin_id
    )
    .execute(&state.db)
    .await
    .unwrap();

    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/characters/esi-search?q=wasp",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["data"]["unavailable"], true);
    assert!(body["data"]["results"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn esi_search_unauthenticated_401(pool: PgPool) {
    let state = build_state(pool);
    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/characters/esi-search?q=wasp",
            None,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn esi_search_non_admin_403(pool: PgPool) {
    let state = build_state(pool);
    let (_id, cookie) = session_for(&state, false).await;
    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/characters/esi-search?q=wasp",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── grant / revoke ─────────────────────────────────────────────────────────────

#[sqlx::test]
async fn grant_then_revoke_admin(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;
    let target = accounts::create_account(&state.db).await.unwrap();

    let grant = backend::build_router(state.clone())
        .oneshot(req(
            Method::POST,
            &format!("/api/v1/admin/accounts/{target}/grant-admin"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(grant.status(), StatusCode::NO_CONTENT);
    assert!(
        accounts::get_account(&state.db, target)
            .await
            .unwrap()
            .unwrap()
            .is_server_admin
    );

    let revoke = backend::build_router(state.clone())
        .oneshot(req(
            Method::POST,
            &format!("/api/v1/admin/accounts/{target}/revoke-admin"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::NO_CONTENT);
    assert!(
        !accounts::get_account(&state.db, target)
            .await
            .unwrap()
            .unwrap()
            .is_server_admin
    );
}

#[sqlx::test]
async fn hard_delete_preview_then_delete(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;
    let target = accounts::create_account(&state.db).await.unwrap();

    // Preview reports the (zero-everything) blast radius for the bare account.
    let preview = backend::build_router(state.clone())
        .oneshot(req(
            Method::GET,
            &format!("/api/v1/admin/accounts/{target}/hard-delete-preview"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let body = json_body(preview).await;
    assert_eq!(body["data"]["characters"], 0);
    assert_eq!(body["data"]["owned_maps"], 0);

    // Execute the hard-delete.
    let del = backend::build_router(state.clone())
        .oneshot(req(
            Method::POST,
            &format!("/api/v1/admin/accounts/{target}/hard-delete"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::OK);

    // The account is gone.
    assert!(
        accounts::get_account(&state.db, target)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn hard_delete_preview_404_for_missing(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;
    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            &format!(
                "/api/v1/admin/accounts/{}/hard-delete-preview",
                Uuid::new_v4()
            ),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn grant_admin_404_for_missing_account(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;
    let resp = backend::build_router(state)
        .oneshot(req(
            Method::POST,
            &format!("/api/v1/admin/accounts/{}/grant-admin", Uuid::new_v4()),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn revoke_last_admin_is_409(pool: PgPool) {
    let state = build_state(pool);
    // The session admin is the only admin — revoking itself must 409.
    let (admin_id, cookie) = session_for(&state, true).await;
    let resp = backend::build_router(state)
        .oneshot(req(
            Method::POST,
            &format!("/api/v1/admin/accounts/{admin_id}/revoke-admin"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_body(resp).await["error"]["code"],
        "cannot_remove_last_server_admin"
    );
}

// ── blocks ─────────────────────────────────────────────────────────────────────

#[sqlx::test]
async fn block_then_list_then_unblock(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;

    // Block an unowned id (snapshot fetch fails fast -> null name/corp).
    let block = backend::build_router(state.clone())
        .oneshot(req(
            Method::POST,
            "/api/v1/admin/blocks",
            Some(&cookie),
            Some(json!({"eve_character_id": 4242, "reason": "griefing"})),
        ))
        .await
        .unwrap();
    assert_eq!(block.status(), StatusCode::NO_CONTENT);
    assert!(
        blocks::is_eve_character_blocked(&state.db, 4242)
            .await
            .unwrap()
    );

    // List shows it.
    let list = backend::build_router(state.clone())
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/blocks",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body = json_body(list).await;
    assert_eq!(body["data"][0]["eve_character_id"], 4242);
    assert_eq!(body["data"][0]["reason"], "griefing");

    // Unblock.
    let unblock = backend::build_router(state.clone())
        .oneshot(req(
            Method::DELETE,
            "/api/v1/admin/blocks/4242",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(unblock.status(), StatusCode::NO_CONTENT);
    assert!(
        !blocks::is_eve_character_blocked(&state.db, 4242)
            .await
            .unwrap()
    );
}

#[sqlx::test]
async fn block_own_character_is_409(pool: PgPool) {
    let state = build_state(pool);
    let (admin_id, cookie) = session_for(&state, true).await;
    // A character on the admin's own account.
    sqlx::query!(
        "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name)
         VALUES ($1, 3333, 'Admin Alt', 1, 'Corp')",
        admin_id
    )
    .execute(&state.db)
    .await
    .unwrap();

    let resp = backend::build_router(state)
        .oneshot(req(
            Method::POST,
            "/api/v1/admin/blocks",
            Some(&cookie),
            Some(json!({"eve_character_id": 3333, "reason": null})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(json_body(resp).await["error"]["code"], "cannot_block_self");
}

#[sqlx::test]
async fn unblock_404_when_not_blocked(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;
    let resp = backend::build_router(state)
        .oneshot(req(
            Method::DELETE,
            "/api/v1/admin/blocks/999999",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── audit list + filter + pagination ───────────────────────────────────────────

#[sqlx::test]
async fn audit_list_filters_and_paginates(pool: PgPool) {
    let state = build_state(pool);
    let (_admin, cookie) = session_for(&state, true).await;

    // Generate audit rows: block two distinct characters.
    for id in [100_i64, 200] {
        backend::build_router(state.clone())
            .oneshot(req(
                Method::POST,
                "/api/v1/admin/blocks",
                Some(&cookie),
                Some(json!({ "eve_character_id": id, "reason": "x" })),
            ))
            .await
            .unwrap();
    }

    // Unfiltered: newest-first, with a next_before cursor.
    let resp = backend::build_router(state.clone())
        .oneshot(req(Method::GET, "/api/v1/admin/audit", Some(&cookie), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    let entries = body["data"]["entries"].as_array().unwrap();
    assert!(entries.len() >= 2);
    assert_eq!(entries[0]["event_type"], "eve_character_blocked");
    assert!(body["data"]["next_before"].is_string());

    // Filter by event_type + target_id.
    let resp = backend::build_router(state.clone())
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/audit?event_type=eve_character_blocked&target_id=200",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    let body = json_body(resp).await;
    let entries = body["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["target_id"], "200");
}

#[sqlx::test]
async fn audit_target_name_filter_is_case_insensitive(pool: PgPool) {
    let state = build_state(pool);
    let (admin_id, cookie) = session_for(&state, true).await;
    // Grant admin to a target account that has a main — the grant audit row's
    // target_name is the target account's main character name.
    let target = accounts::create_account(&state.db).await.unwrap();
    sqlx::query!(
        "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name, is_main)
         VALUES ($1, 5005, 'Boss Pilot', 1, 'Corp', TRUE)",
        target
    )
    .execute(&state.db)
    .await
    .unwrap();
    let _ = admin_id;

    backend::build_router(state.clone())
        .oneshot(req(
            Method::POST,
            &format!("/api/v1/admin/accounts/{target}/grant-admin"),
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();

    let resp = backend::build_router(state.clone())
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/audit?target_name=boss%20pilot",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    let body = json_body(resp).await;
    let entries = body["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "case-insensitive target_name match");
    assert_eq!(entries[0]["event_type"], "server_admin_granted");
    assert_eq!(entries[0]["target_name"], "Boss Pilot");
}

// ── auth gating (per-endpoint, beyond the coverage test) ───────────────────────

#[sqlx::test]
async fn admin_endpoint_rejects_non_admin_403(pool: PgPool) {
    let state = build_state(pool);
    let (_id, cookie) = session_for(&state, false).await;
    let resp = backend::build_router(state)
        .oneshot(req(
            Method::GET,
            "/api/v1/admin/accounts",
            Some(&cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        json_body(resp).await["error"]["code"],
        "forbidden_admin_required"
    );
}

#[sqlx::test]
async fn admin_endpoint_rejects_unauthenticated_401(pool: PgPool) {
    let state = build_state(pool);
    let resp = backend::build_router(state)
        .oneshot(req(Method::GET, "/api/v1/admin/accounts", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn admin_endpoint_rejects_bearer_key_401(pool: PgPool) {
    let state = build_state(pool);
    let (admin_id, _cookie) = session_for(&state, true).await;
    let key = backend::services::api_keys::create_key(&state.db, admin_id, "k", None)
        .await
        .unwrap()
        .plaintext;

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/admin/accounts")
                .header(header::AUTHORIZATION, format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Cookie-only extractor — a bearer key (even for an admin account) is a 401.
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
