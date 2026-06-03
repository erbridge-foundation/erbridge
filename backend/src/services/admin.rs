use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent, AuditLogEntry, ServerAdminGrantSource},
    db::{accounts, blocks, characters, sessions},
    error::{AppError, ConflictKind},
    esi::{self, search::SearchCategory},
    services::entity_search::{self, EntitySearchOutcome},
};

/// Re-exported so the admin handler keeps importing `EsiSearchContext` from this
/// module; the type and the token-acquisition path it feeds now live in
/// `services::entity_search`, shared with the account-authenticated search.
pub use crate::services::entity_search::EsiSearchContext;

/// A character-search result enriched for the admin UI: the base match plus a
/// deterministic portrait URL and whether the character is already blocked.
pub struct AdminCharacterSearchResult {
    pub eve_character_id: i64,
    pub name: String,
    pub is_main: bool,
    pub account_id: Option<Uuid>,
    pub portrait_url: String,
    pub already_blocked: bool,
}

/// A character matched via ESI (no local account context). Portrait is
/// deterministic; `already_blocked` is annotated from the block list.
pub struct EsiCharacterSearchResult {
    pub eve_character_id: i64,
    pub name: String,
    pub portrait_url: String,
    pub already_blocked: bool,
}

/// The outcome of an ESI character search. `Unavailable` is a graceful,
/// non-error state the handler maps to a `200` with an empty list and an
/// `unavailable` indicator — never a 5xx. It is distinct from
/// `Available(vec![])` ("the search ran and matched nothing").
pub enum EsiSearchOutcome {
    Available(Vec<EsiCharacterSearchResult>),
    Unavailable,
}

/// Default and maximum page size for the admin audit browser. A caller-supplied
/// limit is clamped to `[1, MAX]`; `None` falls back to `DEFAULT`.
const DEFAULT_AUDIT_LIMIT: i64 = 50;
const MAX_AUDIT_LIMIT: i64 = 200;

/// Grants server admin to `target`. Idempotent: granting an already-admin
/// account is a success no-op that emits no audit event. A non-existent target
/// is `NotFound` (404). A state-changing grant emits `ServerAdminGranted`
/// with the `AdminGrant` source, in the same transaction as the flag flip.
///
/// `actor` is the admin performing the grant (the audit actor).
pub async fn grant_admin(pool: &PgPool, actor: Uuid, target: Uuid) -> Result<(), AppError> {
    if !accounts::account_exists(pool, target)
        .await
        .map_err(AppError::Internal)?
    {
        return Err(AppError::NotFound);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let changed = accounts::set_server_admin(&mut tx, target, true)
        .await
        .map_err(AppError::Internal)?;
    if changed {
        audit::record_in_tx(
            &mut tx,
            Some(actor),
            None,
            AuditEvent::ServerAdminGranted {
                account_id: target,
                source: ServerAdminGrantSource::AdminGrant,
            },
        )
        .await
        .map_err(AppError::Internal)?;
    }
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Revokes server admin from `target`. Idempotent: revoking a non-admin is a
/// success no-op (no audit). A non-existent target is `NotFound` (404). The
/// last-admin guard runs INSIDE the transaction: if clearing the flag would
/// drop the active-admin count to zero, the request is rejected with
/// `CannotRemoveLastServerAdmin` (409) and the transaction rolls back.
/// Self-revoke is permitted as long as the guard holds. A state-changing revoke
/// emits `ServerAdminRevoked` in the same transaction.
pub async fn revoke_admin(pool: &PgPool, actor: Uuid, target: Uuid) -> Result<(), AppError> {
    if !accounts::account_exists(pool, target)
        .await
        .map_err(AppError::Internal)?
    {
        return Err(AppError::NotFound);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let changed = accounts::set_server_admin(&mut tx, target, false)
        .await
        .map_err(AppError::Internal)?;

    if !changed {
        // Target was not an admin — idempotent no-op. Nothing was changed, so
        // the rollback/commit are equivalent; commit to release the tx cleanly.
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        return Ok(());
    }

    // Guard runs after the flip, within the tx, so the count reflects the
    // pending change. Zero active admins remaining → reject and roll back.
    let remaining = accounts::count_server_admins_tx(&mut tx)
        .await
        .map_err(AppError::Internal)?;
    if remaining == 0 {
        tx.rollback()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        return Err(AppError::Conflict(
            ConflictKind::CannotRemoveLastServerAdmin,
        ));
    }

    audit::record_in_tx(
        &mut tx,
        Some(actor),
        None,
        AuditEvent::ServerAdminRevoked { account_id: target },
    )
    .await
    .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Blocks `eve_character_id`. The name/corp snapshot is pre-fetched by the
/// handler (best-effort from ESI — `None` when unavailable) so this service
/// stays HTTP-free, mirroring `complete_sso_callback`'s split.
///
/// Self-block guard: an admin SHALL NOT block any character belonging to their
/// own account → `CannotBlockSelf` (409), writing nothing.
///
/// Idempotent: blocking an already-blocked character is a success no-op with no
/// audit event and no teardown.
///
/// When the character resolves to an account, the same transaction clears that
/// account's EVE tokens and deletes all its sessions. A state-changing block
/// emits `EveCharacterBlocked` in the transaction.
pub async fn block_character(
    pool: &PgPool,
    actor: Uuid,
    eve_character_id: i64,
    reason: Option<&str>,
    character_name: Option<&str>,
    corporation_name: Option<&str>,
) -> Result<(), AppError> {
    // Resolve the owning account (if any) up front — needed for both the
    // self-block guard and the teardown decision.
    let owning_account = characters::find_account_for_eve_character(pool, eve_character_id)
        .await
        .map_err(AppError::Internal)?;

    // Self-block guard: reject if the target character belongs to the actor's
    // own account. Writes nothing.
    if owning_account == Some(actor) {
        return Err(AppError::Conflict(ConflictKind::CannotBlockSelf));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let inserted = blocks::insert_block(
        &mut tx,
        eve_character_id,
        character_name,
        corporation_name,
        reason,
        actor,
    )
    .await
    .map_err(AppError::Internal)?;

    if !inserted {
        // Already blocked — idempotent no-op, no teardown, no audit.
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        return Ok(());
    }

    // Tear down the owning account (if any), mirroring soft-delete: clear EVE
    // tokens and delete all sessions, atomically with the block insert.
    if let Some(account_id) = owning_account {
        characters::clear_tokens_for_account(&mut tx, account_id)
            .await
            .map_err(AppError::Internal)?;
        sessions::delete_for_account_in_tx(&mut tx, account_id)
            .await
            .map_err(AppError::Internal)?;
    }

    audit::record_in_tx(
        &mut tx,
        Some(actor),
        None,
        AuditEvent::EveCharacterBlocked {
            eve_character_id,
            reason: reason.map(|r| r.to_string()),
        },
    )
    .await
    .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Unblocks `eve_character_id`. `NotFound` (404) if no block row matches. A
/// successful unblock emits `EveCharacterUnblocked` in the same transaction.
/// Tokens and sessions are NOT restored — the formerly-blocked account's
/// characters remain `token_status = "expired"` until re-authorised via SSO.
pub async fn unblock_character(
    pool: &PgPool,
    actor: Uuid,
    eve_character_id: i64,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let deleted = blocks::delete_block(&mut tx, eve_character_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        tx.rollback()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        return Err(AppError::NotFound);
    }

    audit::record_in_tx(
        &mut tx,
        Some(actor),
        None,
        AuditEvent::EveCharacterUnblocked { eve_character_id },
    )
    .await
    .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// An account with its characters, as surfaced by the admin accounts list.
/// Re-exported from the db layer so the DTO maps from a service-owned type.
pub use crate::db::accounts::AccountWithCharacters as AdminAccountInfo;

/// All accounts (newest first) each with its characters, for the admin accounts
/// list.
pub async fn list_accounts(pool: &PgPool) -> Result<Vec<AdminAccountInfo>, AppError> {
    accounts::list_accounts_with_characters(pool)
        .await
        .map_err(AppError::Internal)
}

/// All block rows, newest first (pass-through for the admin block list).
pub async fn list_blocks(pool: &PgPool) -> Result<Vec<blocks::BlockedEveCharacter>, AppError> {
    blocks::list_blocks(pool).await.map_err(AppError::Internal)
}

/// Local character name search for the grant + block UIs. `limit` is clamped to
/// the audit page-size bounds for a sane cap. Each result is enriched with a
/// deterministic portrait URL and an `already_blocked` flag (so the block picker
/// can mark pilots already on the list and the grant picker renders portraits).
pub async fn search_characters(
    pool: &PgPool,
    q: &str,
    limit: Option<i64>,
) -> Result<Vec<AdminCharacterSearchResult>, AppError> {
    let limit = clamp_limit(limit);
    let matches = characters::search_by_name(pool, q, limit)
        .await
        .map_err(AppError::Internal)?;

    let mut out = Vec::with_capacity(matches.len());
    for m in matches {
        let already_blocked = blocks::is_eve_character_blocked(pool, m.eve_character_id)
            .await
            .map_err(AppError::Internal)?;
        out.push(AdminCharacterSearchResult {
            portrait_url: esi::portrait_url(m.eve_character_id),
            eve_character_id: m.eve_character_id,
            name: m.name,
            is_main: m.is_main,
            account_id: m.account_id,
            already_blocked,
        });
    }
    Ok(out)
}

/// ESI-backed character name search, performed on behalf of `admin_account_id`'s
/// own main character. Used as the block UI's fallback when a never-seen pilot
/// is not in the local index. Resolves a usable access token (decrypt + a
/// best-effort refresh on expiry), searches ESI (`strict=false` substring),
/// resolves the returned IDs to names, and annotates each with a portrait URL +
/// `already_blocked`. Any token/scope/ESI failure resolves to
/// [`EsiSearchOutcome::Unavailable`] — never an error that becomes a 5xx.
///
/// The caller MUST guarantee `q.len() >= search::MIN_SEARCH_LEN`.
pub async fn esi_search_characters(
    pool: &PgPool,
    ctx: EsiSearchContext<'_>,
    admin_account_id: Uuid,
    q: &str,
    _limit: Option<i64>,
) -> Result<EsiSearchOutcome, AppError> {
    // Delegate to the shared entity search (character category only); the per-
    // category cap lives in that service. The admin contract differs only in
    // its enrichment: a deterministic portrait URL + an `already_blocked` flag.
    let results = match entity_search::search_entities(
        pool,
        &ctx,
        admin_account_id,
        q,
        &[SearchCategory::Character],
    )
    .await?
    {
        EntitySearchOutcome::Available(r) => r,
        EntitySearchOutcome::Unavailable => return Ok(EsiSearchOutcome::Unavailable),
    };

    let mut out = Vec::with_capacity(results.characters.len());
    for c in results.characters {
        let already_blocked = blocks::is_eve_character_blocked(pool, c.eve_character_id)
            .await
            .map_err(AppError::Internal)?;
        out.push(EsiCharacterSearchResult {
            portrait_url: esi::portrait_url(c.eve_character_id),
            eve_character_id: c.eve_character_id,
            name: c.name,
            already_blocked,
        });
    }

    Ok(EsiSearchOutcome::Available(out))
}

/// Audit-log pass-through forwarding every filter axis — including the
/// target-first axes (`target_type` / `target_id` / `target_name`) added by
/// `add-audit-log-target-columns` — plus the `before` keyset cursor and a
/// clamped `limit`.
#[allow(clippy::too_many_arguments)]
pub async fn list_audit_log(
    pool: &PgPool,
    event_type: Option<&str>,
    actor: Option<Uuid>,
    target_type: Option<&str>,
    target_id: Option<&str>,
    target_name: Option<&str>,
    before: Option<DateTime<Utc>>,
    limit: Option<i64>,
) -> Result<Vec<AuditLogEntry>, AppError> {
    let limit = clamp_limit(limit);
    audit::list_audit_log(
        pool,
        event_type,
        actor,
        target_type,
        target_id,
        target_name,
        before,
        limit,
    )
    .await
    .map_err(AppError::Internal)
}

/// Clamps a caller-supplied page size into `[1, MAX_AUDIT_LIMIT]`, defaulting
/// when `None`.
fn clamp_limit(limit: Option<i64>) -> i64 {
    match limit {
        None => DEFAULT_AUDIT_LIMIT,
        Some(n) => n.clamp(1, MAX_AUDIT_LIMIT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── clamp_limit (pure) ────────────────────────────────────────────────────

    #[test]
    fn clamp_limit_defaults_when_none() {
        assert_eq!(clamp_limit(None), DEFAULT_AUDIT_LIMIT);
    }

    #[test]
    fn clamp_limit_caps_at_max() {
        assert_eq!(clamp_limit(Some(10_000)), MAX_AUDIT_LIMIT);
    }

    #[test]
    fn clamp_limit_floors_at_one() {
        assert_eq!(clamp_limit(Some(0)), 1);
        assert_eq!(clamp_limit(Some(-5)), 1);
    }

    #[test]
    fn clamp_limit_passes_through_in_range() {
        assert_eq!(clamp_limit(Some(20)), 20);
    }

    // ── DB-backed service tests ─────────────────────────────────────────────────

    use crate::db::characters as char_db;
    use chrono::Utc;

    fn key() -> Vec<u8> {
        vec![0u8; 32]
    }

    /// Creates an account, optionally admin, with a main character bound to
    /// `eve_id` (so audit main-snapshots resolve and the block teardown has
    /// something to clear).
    async fn account_with_main(pool: &PgPool, eve_id: i64, name: &str, admin: bool) -> Uuid {
        let account_id = accounts::create_account(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        if admin {
            accounts::set_server_admin(&mut tx, account_id, true)
                .await
                .unwrap();
        }
        let char_id = char_db::upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            name,
            1_000_001,
            "Corp",
            None,
            None,
            "client",
            "access",
            "refresh",
            Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &key(),
        )
        .await
        .unwrap();
        char_db::promote_if_no_main(&mut tx, account_id, char_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        account_id
    }

    async fn audit_count(pool: &PgPool, event_type: &str) -> i64 {
        sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM audit_log WHERE event_type = $1",
            event_type
        )
        .fetch_one(pool)
        .await
        .unwrap()
        .c
    }

    #[sqlx::test]
    async fn grant_admin_sets_flag_and_emits_once(pool: PgPool) {
        let actor = account_with_main(&pool, 1, "Admin", true).await;
        let target = account_with_main(&pool, 2, "Target", false).await;

        grant_admin(&pool, actor, target).await.unwrap();
        assert!(
            accounts::get_account(&pool, target)
                .await
                .unwrap()
                .unwrap()
                .is_server_admin
        );
        assert_eq!(audit_count(&pool, "server_admin_granted").await, 1);

        // Idempotent: second grant is a no-op, no new audit row.
        grant_admin(&pool, actor, target).await.unwrap();
        assert_eq!(audit_count(&pool, "server_admin_granted").await, 1);
    }

    #[sqlx::test]
    async fn grant_admin_404_for_missing_account(pool: PgPool) {
        let actor = account_with_main(&pool, 1, "Admin", true).await;
        let err = grant_admin(&pool, actor, Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound));
    }

    #[sqlx::test]
    async fn revoke_admin_clears_flag_and_emits(pool: PgPool) {
        let actor = account_with_main(&pool, 1, "Admin", true).await;
        let other = account_with_main(&pool, 2, "Other", true).await;

        revoke_admin(&pool, actor, other).await.unwrap();
        assert!(
            !accounts::get_account(&pool, other)
                .await
                .unwrap()
                .unwrap()
                .is_server_admin
        );
        assert_eq!(audit_count(&pool, "server_admin_revoked").await, 1);
    }

    #[sqlx::test]
    async fn revoke_admin_last_admin_guard_rejects_and_preserves_flag(pool: PgPool) {
        // Exactly one admin — revoking it (incl. self) must 409 and roll back.
        let only = account_with_main(&pool, 1, "Only", true).await;
        let err = revoke_admin(&pool, only, only).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotRemoveLastServerAdmin)
        ));
        assert!(
            accounts::get_account(&pool, only)
                .await
                .unwrap()
                .unwrap()
                .is_server_admin,
            "flag must be unchanged after the guard rolls back"
        );
        assert_eq!(audit_count(&pool, "server_admin_revoked").await, 0);
    }

    #[sqlx::test]
    async fn revoke_self_allowed_when_not_last(pool: PgPool) {
        let a = account_with_main(&pool, 1, "A", true).await;
        let _b = account_with_main(&pool, 2, "B", true).await;
        // Two admins exist — a may revoke itself.
        revoke_admin(&pool, a, a).await.unwrap();
        assert!(
            !accounts::get_account(&pool, a)
                .await
                .unwrap()
                .unwrap()
                .is_server_admin
        );
    }

    #[sqlx::test]
    async fn revoke_non_admin_is_noop(pool: PgPool) {
        let actor = account_with_main(&pool, 1, "Admin", true).await;
        let target = account_with_main(&pool, 2, "Plain", false).await;
        revoke_admin(&pool, actor, target).await.unwrap();
        assert_eq!(audit_count(&pool, "server_admin_revoked").await, 0);
    }

    #[sqlx::test]
    async fn block_with_account_clears_tokens_and_kills_sessions(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        let victim = account_with_main(&pool, 500, "Victim", false).await;
        // Give the victim a live session.
        sqlx::query!(
            "INSERT INTO session (session_id, account_id, expires_at)
             VALUES ('vsess', $1, now() + interval '7 days')",
            victim
        )
        .execute(&pool)
        .await
        .unwrap();

        block_character(
            &pool,
            admin,
            500,
            Some("griefing"),
            Some("Victim"),
            Some("Corp"),
        )
        .await
        .unwrap();

        // Block row exists.
        assert!(blocks::is_eve_character_blocked(&pool, 500).await.unwrap());
        // Tokens cleared on the owned character.
        let chars = char_db::list_for_account(&pool, victim).await.unwrap();
        assert!(
            chars.iter().all(|c| c.encrypted_refresh_token.is_none()),
            "owned character tokens must be cleared"
        );
        // Sessions deleted.
        assert!(
            sessions::list_ids_for_account(&pool, victim)
                .await
                .unwrap()
                .is_empty(),
            "owned account sessions must be deleted"
        );
        assert_eq!(audit_count(&pool, "eve_character_blocked").await, 1);
    }

    #[sqlx::test]
    async fn block_without_account_is_a_bare_insert(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        // 999 resolves to no account.
        block_character(&pool, admin, 999, None, None, None)
            .await
            .unwrap();
        assert!(blocks::is_eve_character_blocked(&pool, 999).await.unwrap());
        assert_eq!(audit_count(&pool, "eve_character_blocked").await, 1);
    }

    #[sqlx::test]
    async fn block_is_idempotent(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        block_character(&pool, admin, 999, None, None, None)
            .await
            .unwrap();
        // Second block of the same id is a no-op, no second audit row.
        block_character(&pool, admin, 999, None, None, None)
            .await
            .unwrap();
        assert_eq!(audit_count(&pool, "eve_character_blocked").await, 1);
    }

    #[sqlx::test]
    async fn block_self_is_rejected_and_writes_nothing(pool: PgPool) {
        let admin = account_with_main(&pool, 7, "Self Admin", true).await;
        // 7 belongs to the admin's own account.
        let err = block_character(&pool, admin, 7, Some("oops"), None, None)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotBlockSelf)
        ));
        assert!(!blocks::is_eve_character_blocked(&pool, 7).await.unwrap());
        assert_eq!(audit_count(&pool, "eve_character_blocked").await, 0);
    }

    #[sqlx::test]
    async fn unblock_removes_row_and_emits(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        block_character(&pool, admin, 999, None, None, None)
            .await
            .unwrap();
        unblock_character(&pool, admin, 999).await.unwrap();
        assert!(!blocks::is_eve_character_blocked(&pool, 999).await.unwrap());
        assert_eq!(audit_count(&pool, "eve_character_unblocked").await, 1);
    }

    #[sqlx::test]
    async fn unblock_404_when_not_blocked(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        let err = unblock_character(&pool, admin, 12345).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound));
        assert_eq!(audit_count(&pool, "eve_character_unblocked").await, 0);
    }

    // ── character search enrichment + ESI search ────────────────────────────────

    use reqwest_middleware::ClientWithMiddleware;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn http() -> ClientWithMiddleware {
        reqwest::Client::new().into()
    }

    /// A JWT-shaped access token carrying the given owner hash, for mocking the
    /// SSO refresh endpoint (the refresh path parses the `owner` claim).
    fn refresh_access_jwt(owner: &str) -> String {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            format!(r#"{{"sub":"CHARACTER:EVE:42","name":"Self","owner":"{owner}","scp":"a"}}"#)
                .as_bytes(),
        );
        format!("{header}.{payload}.sig")
    }

    /// An ESI context pointing search/resolve at `esi_base` and token refresh at
    /// `token_endpoint`. Uses the all-zero test key matching `account_with_main`.
    fn ctx<'a>(
        http: &'a ClientWithMiddleware,
        esi_base: &'a str,
        token_endpoint: &'a str,
    ) -> EsiSearchContext<'a> {
        EsiSearchContext {
            http,
            esi_base_url: esi_base,
            token_endpoint,
            client_id: "client",
            client_secret: "secret",
            encryption_key: KEY,
        }
    }

    const KEY: &[u8] = &[0u8; 32];

    /// Seeds an account whose main character has an *expired* access token, so
    /// the search path must refresh. Returns the account id.
    async fn account_with_expired_main(pool: &PgPool, eve_id: i64) -> Uuid {
        let account_id = accounts::create_account(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let char_id = char_db::upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            "Expired",
            1,
            "Corp",
            None,
            None,
            "client",
            "access",
            "refresh",
            Utc::now() - chrono::Duration::hours(1),
            &[],
            "owner-hash",
            KEY,
        )
        .await
        .unwrap();
        char_db::promote_if_no_main(&mut tx, account_id, char_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        account_id
    }

    #[sqlx::test]
    async fn search_characters_enriches_with_portrait_and_blocked_flag(pool: PgPool) {
        // Two searchable characters; block one of them via a different admin.
        account_with_main(&pool, 100, "Findme", false).await;
        account_with_main(&pool, 200, "Findme Two", false).await;
        let blocker = account_with_main(&pool, 300, "Blocker", true).await;
        block_character(&pool, blocker, 200, None, None, None)
            .await
            .unwrap();

        let results = search_characters(&pool, "Findme", None).await.unwrap();
        let two = results
            .iter()
            .find(|r| r.eve_character_id == 200)
            .expect("Findme Two present");
        assert!(two.already_blocked);
        assert!(two.portrait_url.contains("/characters/200/portrait"));

        let one = results
            .iter()
            .find(|r| r.eve_character_id == 100)
            .expect("Findme present");
        assert!(!one.already_blocked);
    }

    #[sqlx::test]
    async fn esi_search_unavailable_when_admin_has_no_main(pool: PgPool) {
        // Account with no characters → no main token material → Unavailable.
        let admin = accounts::create_account(&pool).await.unwrap();
        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, "http://unused", "http://unused"),
            admin,
            "wasp",
            None,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, EsiSearchOutcome::Unavailable));
    }

    #[sqlx::test]
    async fn esi_search_available_resolves_and_annotates(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        // Pre-block id 555 so the annotation is observable.
        block_character(&pool, admin, 555, None, None, None)
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "character": [555, 777]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/characters/555/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "name": "Blocked Pilot" })),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/characters/777/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "name": "Free Pilot" })),
            )
            .mount(&server)
            .await;

        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, &server.uri(), "http://unused"),
            admin,
            "pilot",
            None,
        )
        .await
        .unwrap();
        let results = match outcome {
            EsiSearchOutcome::Available(r) => r,
            EsiSearchOutcome::Unavailable => panic!("expected Available"),
        };
        assert_eq!(results.len(), 2);
        let blocked = results.iter().find(|r| r.eve_character_id == 555).unwrap();
        assert_eq!(blocked.name, "Blocked Pilot");
        assert!(blocked.already_blocked);
        assert!(blocked.portrait_url.contains("/characters/555/portrait"));
        let free = results.iter().find(|r| r.eve_character_id == 777).unwrap();
        assert!(!free.already_blocked);
    }

    #[sqlx::test]
    async fn esi_search_empty_is_available_not_unavailable(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, &server.uri(), "http://unused"),
            admin,
            "zzz",
            None,
        )
        .await
        .unwrap();
        match outcome {
            EsiSearchOutcome::Available(r) => assert!(r.is_empty()),
            EsiSearchOutcome::Unavailable => {
                panic!("empty result must be Available, not Unavailable")
            }
        }
    }

    #[sqlx::test]
    async fn esi_search_unavailable_when_esi_rejects(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/characters/1/search/"))
            .respond_with(ResponseTemplate::new(403)) // missing scope
            .mount(&server)
            .await;

        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, &server.uri(), "http://unused"),
            admin,
            "wasp",
            None,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, EsiSearchOutcome::Unavailable));
    }

    #[sqlx::test]
    async fn esi_search_refreshes_expired_token_then_searches(pool: PgPool) {
        let admin = account_with_expired_main(&pool, 42).await;

        let server = MockServer::start().await;
        // Token endpoint returns a fresh token set. The access token must be a
        // JWT carrying the `owner` claim — the refresh path now parses it.
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": refresh_access_jwt("owner-hash"),
                "refresh_token": "fresh-refresh",
                "expires_in": 1200
            })))
            .mount(&server)
            .await;
        // ESI search succeeds (any bearer accepted by the mock).
        Mock::given(method("GET"))
            .and(path("/characters/42/search/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "character": [42] })),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/characters/42/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "Self" })),
            )
            .mount(&server)
            .await;

        let token_endpoint = format!("{}/oauth/token", server.uri());
        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, &server.uri(), &token_endpoint),
            admin,
            "sel",
            None,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, EsiSearchOutcome::Available(_)));

        // The rotated tokens were persisted: expiry moved into the future.
        let material = char_db::get_main_token_material(&pool, admin)
            .await
            .unwrap()
            .unwrap();
        assert!(material.access_token_expires_at.unwrap() > Utc::now());
    }

    #[sqlx::test]
    async fn esi_search_unavailable_when_expired_and_refresh_rejected(pool: PgPool) {
        let admin = account_with_expired_main(&pool, 42).await;
        let server = MockServer::start().await;
        // Refresh is rejected (e.g. revoked refresh token).
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&server)
            .await;

        let token_endpoint = format!("{}/oauth/token", server.uri());
        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, &server.uri(), &token_endpoint),
            admin,
            "sel",
            None,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, EsiSearchOutcome::Unavailable));
    }
}
