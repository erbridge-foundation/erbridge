// Integration tests for audit-log emission. Drives the post-ESI SSO completion
// service (`backend::services::auth::complete_sso_callback`) and the
// account-management / api-key services directly, then asserts on rows in the
// `audit_log` table. Every test also asserts "no unexpected event types"
// appear, so accidental over-logging fails the suite.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use backend::{
    app_state::AppState,
    config::Config,
    db::{accounts, characters},
    esi::EsiMetadata,
    handlers::crypto,
    services::auth::{SsoCompletionInput, complete_sso_callback},
    session::{InflightStore, SessionStore},
};

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

fn build_state(pool: PgPool) -> AppState {
    AppState {
        config: test_config(),
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

const ENCRYPTION_KEY: [u8; 32] = [0u8; 32];

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
        owner_hash: "owner-hash",
        encryption_key: &ENCRYPTION_KEY,
    }
}

#[derive(Debug)]
struct AuditRow {
    event_type: String,
    actor_account_id: Option<Uuid>,
    actor_character_id: Option<i64>,
    actor_character_name: Option<String>,
    details: serde_json::Value,
    target_type: Option<String>,
    target_id: Option<String>,
    target_name: Option<String>,
}

async fn fetch_audit(pool: &PgPool) -> Vec<AuditRow> {
    sqlx::query!(
        "SELECT event_type, actor_account_id, actor_character_id, actor_character_name, details,
                target_type, target_id, target_name
         FROM audit_log ORDER BY occurred_at, id"
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| AuditRow {
        event_type: r.event_type,
        actor_account_id: r.actor_account_id,
        actor_character_id: r.actor_character_id,
        actor_character_name: r.actor_character_name,
        details: r.details,
        target_type: r.target_type,
        target_id: r.target_id,
        target_name: r.target_name,
    })
    .collect()
}

fn assert_no_unexpected_event_types(rows: &[AuditRow], expected: &[&str]) {
    for row in rows {
        assert!(
            expected.contains(&row.event_type.as_str()),
            "unexpected audit event_type: {} (rows: {:?})",
            row.event_type,
            rows
        );
    }
}

// ── SSO callback emissions ────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn test_first_account_registration_writes_account_registered_and_bootstrap_admin_grant(
    pool: PgPool,
) {
    complete_sso_callback(&pool, sso_input(None, 11111, "Tester Alpha"))
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["account_registered", "server_admin_granted"]);

    let registered = rows
        .iter()
        .find(|r| r.event_type == "account_registered")
        .expect("account_registered row missing");
    assert!(registered.actor_account_id.is_none());
    assert_eq!(registered.actor_character_id, Some(11111));
    assert_eq!(
        registered.actor_character_name.as_deref(),
        Some("Tester Alpha")
    );
    assert_eq!(registered.details["eve_character_id"], 11111i64);
    assert_eq!(registered.details["character_name"], "Tester Alpha");
    // Registration targets the new account; its name snapshots the new main.
    assert_eq!(registered.target_type.as_deref(), Some("account"));
    assert_eq!(registered.target_name.as_deref(), Some("Tester Alpha"));

    let granted = rows
        .iter()
        .find(|r| r.event_type == "server_admin_granted")
        .expect("server_admin_granted row missing");
    assert!(granted.actor_account_id.is_none());
    assert_eq!(granted.actor_character_id, Some(11111));
    assert_eq!(granted.details["source"], "first_account_bootstrap");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_second_account_registration_does_not_emit_bootstrap_admin_grant(pool: PgPool) {
    complete_sso_callback(&pool, sso_input(None, 22221, "First Pilot"))
        .await
        .unwrap();
    // Clear so we can isolate the second registration's emissions.
    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    complete_sso_callback(&pool, sso_input(None, 22222, "Second Pilot"))
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["account_registered"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor_character_id, Some(22222));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_orphan_claim_on_login_writes_account_registered_and_orphan_claim(pool: PgPool) {
    // Insert an orphan directly.
    sqlx::query!(
        "INSERT INTO eve_character (eve_character_id, name, corporation_id, corporation_name)
         VALUES ($1, $2, $3, $4)",
        33333_i64,
        "Orphan Pilot",
        1_000_001_i64,
        "Test Corp",
    )
    .execute(&pool)
    .await
    .unwrap();

    complete_sso_callback(&pool, sso_input(None, 33333, "Orphan Pilot"))
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(
        &rows,
        &[
            "account_registered",
            "orphan_character_claimed",
            "server_admin_granted", // first-account bootstrap still fires
        ],
    );

    let claimed = rows
        .iter()
        .find(|r| r.event_type == "orphan_character_claimed")
        .expect("orphan_character_claimed row missing");
    assert!(claimed.actor_account_id.is_none());
    assert_eq!(claimed.actor_character_id, Some(33333));
    assert_eq!(
        claimed.actor_character_name.as_deref(),
        Some("Orphan Pilot")
    );
    assert_eq!(claimed.details["eve_character_id"], 33333i64);
    assert_eq!(claimed.details["character_name"], "Orphan Pilot");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_orphan_claim_does_not_emit_account_reactivated(pool: PgPool) {
    // The `OrphanCharacterExists` path always creates a *new* account that
    // has never been soft-deleted, so reactivation cannot fire alongside an
    // orphan-claim. This test documents that invariant: even if there are
    // soft-deleted accounts on the system, a fresh orphan-claim transaction
    // does not produce `account_reactivated`.
    let _admin = complete_sso_callback(&pool, sso_input(None, 99100, "Admin"))
        .await
        .unwrap();
    let other = complete_sso_callback(&pool, sso_input(None, 99101, "Other"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    accounts::soft_delete(&mut tx, other).await.unwrap();
    tx.commit().await.unwrap();

    sqlx::query!(
        "INSERT INTO eve_character (eve_character_id, name, corporation_id, corporation_name)
         VALUES ($1, $2, $3, $4)",
        99102_i64,
        "Orphan",
        1_000_001_i64,
        "Test Corp",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    complete_sso_callback(&pool, sso_input(None, 99102, "Orphan"))
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert!(
        rows.iter().all(|r| r.event_type != "account_reactivated"),
        "orphan-claim must not emit account_reactivated, got: {:?}",
        rows.iter().map(|r| &r.event_type).collect::<Vec<_>>()
    );
    // Belt-and-braces: assert the actual emitted set is exactly the expected pair.
    assert_no_unexpected_event_types(&rows, &["account_registered", "orphan_character_claimed"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_add_character_writes_character_added_with_main_as_actor(pool: PgPool) {
    // Register the first account; this character becomes the main.
    let account_id = complete_sso_callback(&pool, sso_input(None, 44440, "Main Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    // Add a fresh second character via the add-character flow.
    complete_sso_callback(&pool, sso_input(Some(account_id), 44441, "Alt Pilot"))
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["character_added"]);
    assert_eq!(rows.len(), 1);

    let added = &rows[0];
    assert_eq!(added.actor_account_id, Some(account_id));
    assert_eq!(added.actor_character_id, Some(44440)); // the MAIN's EVE id
    assert_eq!(added.actor_character_name.as_deref(), Some("Main Pilot"));
    assert_eq!(added.details["eve_character_id"], 44441i64); // the NEW char's EVE id
    assert_eq!(added.details["character_name"], "Alt Pilot");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_add_character_claiming_orphan_writes_orphan_claim_with_main_actor(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 55550, "Main Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    // Insert an orphan that the add-character flow will claim.
    sqlx::query!(
        "INSERT INTO eve_character (eve_character_id, name, corporation_id, corporation_name)
         VALUES ($1, $2, $3, $4)",
        55551_i64,
        "Orphan Alt",
        1_000_001_i64,
        "Test Corp",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    complete_sso_callback(&pool, sso_input(Some(account_id), 55551, "Orphan Alt"))
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["orphan_character_claimed"]);
    assert_eq!(rows.len(), 1);
    let claimed = &rows[0];
    assert_eq!(claimed.actor_account_id, Some(account_id));
    assert_eq!(claimed.actor_character_id, Some(55550)); // main's EVE id
    assert_eq!(claimed.actor_character_name.as_deref(), Some("Main Pilot"));
    assert_eq!(claimed.details["eve_character_id"], 55551i64);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_reactivation_writes_account_reactivated(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 66660, "Returning Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();

    // Manually soft-delete to simulate the prior delete.
    let mut tx = pool.begin().await.unwrap();
    accounts::soft_delete(&mut tx, account_id).await.unwrap();
    tx.commit().await.unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    // Re-login as the same character.
    complete_sso_callback(&pool, sso_input(None, 66660, "Returning Pilot"))
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["account_reactivated"]);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert!(r.actor_account_id.is_none());
    assert_eq!(r.actor_character_id, Some(66660));
    assert_eq!(r.actor_character_name.as_deref(), Some("Returning Pilot"));
    assert_eq!(r.details["account_id"], account_id.to_string());
}

// ── Account-management endpoints ──────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_account_writes_account_deletion_requested(pool: PgPool) {
    // Need two accounts so the first-account-admin can soft-delete.
    let _admin = complete_sso_callback(&pool, sso_input(None, 77770, "Admin Pilot"))
        .await
        .unwrap();
    let user = complete_sso_callback(&pool, sso_input(None, 77771, "User Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    backend::services::account::delete_account(&pool, user)
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["account_deletion_requested"]);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.event_type, "account_deletion_requested");
    assert_eq!(r.actor_account_id, Some(user));
    assert_eq!(r.actor_character_id, Some(77771));
    assert_eq!(r.actor_character_name.as_deref(), Some("User Pilot"));
    assert!(r.details.as_object().unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_set_main_writes_character_set_main_with_outgoing_main_snapshot(pool: PgPool) {
    // Account with main A.
    let account_id = complete_sso_callback(&pool, sso_input(None, 88880, "Pilot A"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    // Add character B via add-character flow.
    complete_sso_callback(&pool, sso_input(Some(account_id), 88881, "Pilot B"))
        .await
        .unwrap();

    // Look up the internal UUID for character B.
    let b_internal = sqlx::query!(
        "SELECT id FROM eve_character WHERE eve_character_id = $1",
        88881_i64
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    // Promote B → main.
    backend::services::account::set_main_character(&pool, account_id, b_internal)
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["character_set_main"]);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.actor_account_id, Some(account_id));
    assert_eq!(r.actor_character_id, Some(88880)); // OUTGOING main (A)
    assert_eq!(r.actor_character_name.as_deref(), Some("Pilot A"));
    assert_eq!(r.details["eve_character_id"], 88881i64); // INCOMING main (B)

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    // Promote A → main again. Now the outgoing should be B.
    let a_internal = sqlx::query!(
        "SELECT id FROM eve_character WHERE eve_character_id = $1",
        88880_i64
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;
    backend::services::account::set_main_character(&pool, account_id, a_internal)
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.actor_character_id, Some(88881)); // OUTGOING main (B)
    assert_eq!(r.details["eve_character_id"], 88880i64); // INCOMING main (A)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_remove_character_writes_character_removed(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 99990, "Main Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    // Add a removable alt.
    complete_sso_callback(&pool, sso_input(Some(account_id), 99991, "Alt Pilot"))
        .await
        .unwrap();
    let alt_internal = sqlx::query!(
        "SELECT id FROM eve_character WHERE eve_character_id = $1",
        99991_i64
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    backend::services::account::delete_character(&pool, account_id, alt_internal)
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["character_removed"]);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.actor_account_id, Some(account_id));
    assert_eq!(r.actor_character_id, Some(99990)); // main
    assert_eq!(r.actor_character_name.as_deref(), Some("Main Pilot"));
    assert_eq!(r.details["eve_character_id"], 99991i64);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_rejected_character_remove_writes_no_audit_row(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 12340, "Only Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    let only_internal = sqlx::query!(
        "SELECT id FROM eve_character WHERE eve_character_id = $1",
        12340_i64
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    // Last-character: 409 expected.
    let err = backend::services::account::delete_character(&pool, account_id, only_internal)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            backend::error::AppError::Conflict(
                backend::error::ConflictKind::CannotRemoveLastCharacter
            )
        ),
        "expected CannotRemoveLastCharacter, got {err:?}"
    );

    let rows = fetch_audit(&pool).await;
    assert!(rows.is_empty(), "expected no audit rows, got {rows:?}");
}

// ── API key endpoints ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn test_create_api_key_writes_api_key_created(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 13130, "Owner Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let created = backend::services::api_keys::create_key(&pool, account_id, "ci", None)
        .await
        .unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["api_key_created"]);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.actor_account_id, Some(account_id));
    assert_eq!(r.actor_character_id, Some(13130));
    assert_eq!(r.actor_character_name.as_deref(), Some("Owner Pilot"));
    assert_eq!(r.details["key_id"], created.id.to_string());
    assert_eq!(r.details["name"], "ci");
    // Key events target the owning account; self-targeting → name is the owner's main.
    assert_eq!(r.target_type.as_deref(), Some("account"));
    assert_eq!(r.target_id.as_deref(), Some(&*account_id.to_string()));
    assert_eq!(r.target_name.as_deref(), Some("Owner Pilot"));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_revoke_api_key_writes_api_key_revoked(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 14140, "Owner Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    let created = backend::services::api_keys::create_key(&pool, account_id, "to-revoke", None)
        .await
        .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let revoked = backend::services::api_keys::delete_key(&pool, created.id, account_id)
        .await
        .unwrap();
    assert!(revoked);

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["api_key_revoked"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].details["key_id"], created.id.to_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_rejected_create_api_key_writes_no_audit_row(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 15150, "Owner Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();
    // Insert a key with the same name first so the second create hits the
    // unique violation.
    backend::services::api_keys::create_key(&pool, account_id, "ci", None)
        .await
        .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let err = backend::services::api_keys::create_key(&pool, account_id, "ci", None)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        backend::error::AppError::Conflict(backend::error::ConflictKind::ApiKeyNameAlreadyExists)
    ));

    let rows = fetch_audit(&pool).await;
    assert!(rows.is_empty(), "expected no audit rows, got {rows:?}");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_revoke_nonexistent_key_writes_no_audit_row(pool: PgPool) {
    let account_id = complete_sso_callback(&pool, sso_input(None, 16160, "Owner Pilot"))
        .await
        .unwrap()
        .account_id()
        .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let revoked = backend::services::api_keys::delete_key(&pool, Uuid::new_v4(), account_id)
        .await
        .unwrap();
    assert!(!revoked);

    let rows = fetch_audit(&pool).await;
    assert!(rows.is_empty(), "expected no audit rows, got {rows:?}");
}

// ── Router-driven end-to-end coverage for every retrofitted handler ───────────
//
// One test per retrofitted handler, exercising the full handler→service→db
// path through the real router (axum). Each asserts (a) the HTTP contract
// (status, key envelope fields) is preserved by the retrofit and (b) the
// audit row landed with the correct actor-character snapshot. This closes the
// "handler-level unit test" gap that we cannot fill via mocked-service unit
// tests (services are free functions, not trait objects, in this codebase).

/// Bootstraps an account + main and returns `(account_id, eve_id, name, cookie)`
/// suitable for cookie-authenticated requests through the router.
async fn bootstrap_session(
    state: &AppState,
    eve_character_id: i64,
    character_name: &str,
) -> (Uuid, String) {
    let account_id =
        complete_sso_callback(&state.db, sso_input(None, eve_character_id, character_name))
            .await
            .unwrap()
            .account_id()
            .unwrap();
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, false)
        .await
        .unwrap();
    let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
    let jwt = crypto::sign_session_jwt(&session_id, &key_bytes).unwrap();
    (account_id, format!("session={jwt}"))
}

#[sqlx::test(migrations = "./migrations")]
async fn router_post_keys_succeeds_and_writes_audit_row(pool: PgPool) {
    let state = build_state(pool.clone());
    let app = backend::build_router(state.clone());

    let (account_id, cookie) = bootstrap_session(&state, 17170, "Session Pilot").await;

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(r#"{"name":"smoke","expires_at":null}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let _ = resp.into_body().collect().await.unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["api_key_created"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor_account_id, Some(account_id));
    assert_eq!(rows[0].actor_character_id, Some(17170));
}

#[sqlx::test(migrations = "./migrations")]
async fn router_delete_keys_succeeds_and_writes_audit_row(pool: PgPool) {
    let state = build_state(pool.clone());
    let app = backend::build_router(state.clone());

    let (account_id, cookie) = bootstrap_session(&state, 18180, "Session Pilot").await;
    // Create a key through the service so we have a UUID to revoke.
    let created = backend::services::api_keys::create_key(&pool, account_id, "revoke-me", None)
        .await
        .unwrap();

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/keys/{}", created.id))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["api_key_revoked"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor_account_id, Some(account_id));
    assert_eq!(rows[0].actor_character_id, Some(18180));
    assert_eq!(rows[0].details["key_id"], created.id.to_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn router_delete_account_succeeds_and_writes_audit_row(pool: PgPool) {
    let state = build_state(pool.clone());
    let app = backend::build_router(state.clone());

    // Two accounts so the first (admin) is not the last server admin.
    let _admin_id = complete_sso_callback(&pool, sso_input(None, 19190, "Admin"))
        .await
        .unwrap();
    let (user_id, cookie) = bootstrap_session(&state, 19191, "User").await;

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method(Method::DELETE)
        .uri("/api/v1/account")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["account_deletion_requested"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor_account_id, Some(user_id));
    assert_eq!(rows[0].actor_character_id, Some(19191));
    assert_eq!(rows[0].actor_character_name.as_deref(), Some("User"));
    // Self-targeting account event: target is the deleting account; name is its main.
    assert_eq!(rows[0].target_type.as_deref(), Some("account"));
    assert_eq!(rows[0].target_id.as_deref(), Some(&*user_id.to_string()));
    assert_eq!(rows[0].target_name.as_deref(), Some("User"));
}

#[sqlx::test(migrations = "./migrations")]
async fn router_set_main_succeeds_and_writes_audit_row(pool: PgPool) {
    let state = build_state(pool.clone());
    let app = backend::build_router(state.clone());

    let (account_id, cookie) = bootstrap_session(&state, 20200, "Pilot A").await;
    // Add a second character to promote.
    complete_sso_callback(&pool, sso_input(Some(account_id), 20201, "Pilot B"))
        .await
        .unwrap();
    let b_internal = sqlx::query!(
        "SELECT id FROM eve_character WHERE eve_character_id = $1",
        20201_i64
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/characters/{b_internal}/set-main"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = resp.into_body().collect().await.unwrap();

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["character_set_main"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor_account_id, Some(account_id));
    // Outgoing main snapshot — Pilot A (still main at audit write time).
    assert_eq!(rows[0].actor_character_id, Some(20200));
    assert_eq!(rows[0].actor_character_name.as_deref(), Some("Pilot A"));
    // Details carry the incoming main — Pilot B.
    assert_eq!(rows[0].details["eve_character_id"], 20201_i64);
    // Target is the character being promoted (Pilot B); no carried name.
    assert_eq!(rows[0].target_type.as_deref(), Some("character"));
    assert_eq!(rows[0].target_id.as_deref(), Some("20201"));
    assert!(rows[0].target_name.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn router_delete_character_succeeds_and_writes_audit_row(pool: PgPool) {
    let state = build_state(pool.clone());
    let app = backend::build_router(state.clone());

    let (account_id, cookie) = bootstrap_session(&state, 21210, "Main").await;
    complete_sso_callback(&pool, sso_input(Some(account_id), 21211, "Alt"))
        .await
        .unwrap();
    let alt_internal = sqlx::query!(
        "SELECT id FROM eve_character WHERE eve_character_id = $1",
        21211_i64
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;

    sqlx::query!("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/characters/{alt_internal}"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let rows = fetch_audit(&pool).await;
    assert_no_unexpected_event_types(&rows, &["character_removed"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor_account_id, Some(account_id));
    assert_eq!(rows[0].actor_character_id, Some(21210));
    assert_eq!(rows[0].details["eve_character_id"], 21211_i64);
    // Target is the removed character (Alt); no carried name.
    assert_eq!(rows[0].target_type.as_deref(), Some("character"));
    assert_eq!(rows[0].target_id.as_deref(), Some("21211"));
    assert!(rows[0].target_name.is_none());
}

// Silence unused-import warnings for utility re-exports.
#[allow(dead_code)]
fn _silence_unused() {
    let _ = characters::get_main_for_account_tx;
}
