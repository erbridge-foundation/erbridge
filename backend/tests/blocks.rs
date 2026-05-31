// Integration tests for block enforcement (section 5 of
// add-server-admin-and-block-list). Exercises the two surviving auth routes —
// the SSO callback (via the completion service) and the bearer branch of
// `AuthenticatedAccount` (via the real router) — plus the cookie-path
// non-enforcement, asserting the soft-delete-mirroring model end to end.

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

use backend::{
    app_state::AppState,
    config::Config,
    db::{accounts, blocks, characters},
    esi::EsiMetadata,
    handlers::crypto,
    services::auth::{SsoCompletionInput, SsoOutcome, complete_sso_callback},
    session::{InflightStore, SessionStore},
};

const TEST_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const ENCRYPTION_KEY: [u8; 32] = [0u8; 32];

fn build_state(pool: PgPool) -> AppState {
    AppState {
        config: Arc::new(Config {
            app_url: "http://localhost:3000".into(),
            encryption_secret: TEST_SECRET.into(),
            esi_client_id: "test_client_id".into(),
            esi_client_secret: "test_client_secret".into(),
            database_url: String::new(),
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

fn sso_input<'a>(
    add_character_account_id: Option<Uuid>,
    eve_character_id: i64,
    character_name: &'a str,
) -> SsoCompletionInput<'a> {
    static EMPTY: Vec<String> = Vec::new();
    SsoCompletionInput {
        add_character_account_id,
        eve_character_id,
        character_name,
        corporation_id: 1_000_001,
        corporation_name: "Test Corp",
        alliance_id: None,
        alliance_name: None,
        esi_client_id: "test_client_id",
        access_token: "fake.access.token",
        refresh_token: "fake.refresh.token",
        access_token_expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        scopes: &EMPTY,
        encryption_key: &ENCRYPTION_KEY,
    }
}

/// Inserts a block row for `eve_character_id`, blocked by `admin`.
async fn block(pool: &PgPool, eve_character_id: i64, admin: Uuid) {
    let mut tx = pool.begin().await.unwrap();
    blocks::insert_block(&mut tx, eve_character_id, None, None, Some("test"), admin)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ── SSO callback enforcement ───────────────────────────────────────────────────

#[sqlx::test]
async fn blocked_character_login_writes_nothing_and_records_rejection(pool: PgPool) {
    // An admin account exists to own the block row (and to keep the block id
    // distinct from any login id).
    let admin = accounts::create_account(&pool).await.unwrap();
    block(&pool, 50001, admin).await;

    let outcome = complete_sso_callback(&pool, sso_input(None, 50001, "Griefer"))
        .await
        .unwrap();
    assert_eq!(outcome, SsoOutcome::Blocked);

    // No eve_character row was written for the blocked id.
    assert_eq!(
        characters::find_account_for_eve_character(&pool, 50001)
            .await
            .unwrap(),
        None,
        "blocked login must not create or bind a character"
    );

    // Exactly one account exists (the admin) — no account was created for the
    // rejected login.
    let account_count = accounts::list_accounts_admin(&pool).await.unwrap().len();
    assert_eq!(
        account_count, 1,
        "rejected login must not create an account"
    );

    // A blocked_login_rejected audit row exists carrying the subject id, with a
    // NULL actor.
    let row = sqlx::query!(
        "SELECT actor_account_id, details FROM audit_log
         WHERE event_type = 'blocked_login_rejected'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.actor_account_id.is_none());
    assert_eq!(row.details["eve_character_id"], 50001_i64);
}

#[sqlx::test]
async fn blocked_character_cannot_be_added_as_alt(pool: PgPool) {
    // An existing (unblocked) account that will attempt to add a blocked alt.
    let owner = complete_sso_callback(&pool, sso_input(None, 60000, "Owner"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    block(&pool, 60001, owner).await;

    let outcome = complete_sso_callback(&pool, sso_input(Some(owner), 60001, "Blocked Alt"))
        .await
        .unwrap();
    assert_eq!(outcome, SsoOutcome::Blocked);

    // The blocked alt was not attached to the owner account.
    assert_eq!(
        characters::find_account_for_eve_character(&pool, 60001)
            .await
            .unwrap(),
        None,
        "blocked character must not be attached as an alt"
    );

    let row = sqlx::query!(
        "SELECT COUNT(*) AS \"count!\" FROM audit_log
         WHERE event_type = 'blocked_login_rejected' AND details->>'eve_character_id' = '60001'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.count, 1);
}

#[sqlx::test]
async fn never_seen_blocked_id_creates_no_orphan_account(pool: PgPool) {
    let admin = accounts::create_account(&pool).await.unwrap();
    block(&pool, 70001, admin).await;

    let outcome = complete_sso_callback(&pool, sso_input(None, 70001, "Never Seen"))
        .await
        .unwrap();
    assert_eq!(outcome, SsoOutcome::Blocked);

    // Only the admin account exists — the block check precedes any account
    // creation, so no orphan account resulted from the rejected login.
    assert_eq!(accounts::list_accounts_admin(&pool).await.unwrap().len(), 1);
}

// ── bearer-branch enforcement ──────────────────────────────────────────────────

/// Mints an account-scoped API key plaintext for `account_id`.
async fn account_key(state: &AppState, account_id: Uuid) -> String {
    backend::services::api_keys::create_key(&state.db, account_id, "test-key", None)
        .await
        .unwrap()
        .plaintext
}

#[sqlx::test]
async fn bearer_request_for_blocked_account_is_rejected(pool: PgPool) {
    let state = build_state(pool.clone());
    // An account that owns a character, with an API key.
    let account_id = complete_sso_callback(&pool, sso_input(None, 80000, "Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    let key = account_key(&state, account_id).await;

    // Block that owned character → the account is now blocked.
    let admin = accounts::create_account(&pool).await.unwrap();
    block(&pool, 80000, admin).await;

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::AUTHORIZATION, format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(resp).await["error"]["code"], "account_blocked");

    // The key row was NOT deleted by the rejection.
    let keys = backend::db::api_keys::list_for_account(&pool, account_id)
        .await
        .unwrap();
    assert_eq!(keys.len(), 1, "block must not delete the API key");
}

#[sqlx::test]
async fn bearer_request_for_non_blocked_account_proceeds(pool: PgPool) {
    let state = build_state(pool.clone());
    let account_id = complete_sso_callback(&pool, sso_input(None, 81000, "Clean Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    let key = account_key(&state, account_id).await;

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::AUTHORIZATION, format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

// ── cookie-path: enforcement is via session deletion, not a per-request check ──

#[sqlx::test]
async fn cookie_request_for_session_less_blocked_account_is_unauthenticated(pool: PgPool) {
    // A blocked account whose sessions were torn down has no live session to
    // present. We simulate the post-block state: the account is blocked and has
    // no session row. A cookie request must 401 `unauthenticated` (the session
    // is gone) — NOT `account_blocked`, because the cookie path performs no
    // block-list check.
    let state = build_state(pool.clone());
    // Establish an owned character so the block binds to a real account; the
    // resolved account id itself isn't needed for the assertion below.
    complete_sso_callback(&pool, sso_input(None, 82000, "Pilot"))
        .await
        .unwrap();
    let admin = accounts::create_account(&pool).await.unwrap();
    block(&pool, 82000, admin).await;
    // No session was ever created for `account_id`, mirroring the post-block
    // state where block deleted them. Forge a cookie for a non-existent session.
    let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
    let jwt = crypto::sign_session_jwt(&Uuid::new_v4().to_string(), &key_bytes).unwrap();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::COOKIE, format!("session={jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(resp).await["error"]["code"],
        "unauthenticated",
        "cookie path reports the missing session, not a block check"
    );
}

#[sqlx::test]
async fn cookie_request_for_non_blocked_account_is_served(pool: PgPool) {
    // The hot cookie path is not taxed by blocking: a non-blocked account with a
    // live session is served normally. (This also demonstrates the cookie branch
    // succeeds without consulting the block list — there is no block row at all.)
    let state = build_state(pool.clone());
    let account_id = accounts::create_account(&pool).await.unwrap();
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .unwrap();
    let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
    let jwt = crypto::sign_session_jwt(&session_id, &key_bytes).unwrap();

    let resp = backend::build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/me")
                .header(header::COOKIE, format!("session={jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
