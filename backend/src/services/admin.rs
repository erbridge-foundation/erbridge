use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent, AuditLogEntry, ServerAdminGrantSource},
    db::{accounts, blocks, characters, sessions},
    error::{AppError, ConflictKind},
};

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

/// Character name search for the grant UI. `limit` is clamped to the audit
/// page-size bounds for a sane cap.
pub async fn search_characters(
    pool: &PgPool,
    q: &str,
    limit: Option<i64>,
) -> Result<Vec<characters::CharacterSearchResult>, AppError> {
    let limit = clamp_limit(limit);
    characters::search_by_name(pool, q, limit)
        .await
        .map_err(AppError::Internal)
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
}
