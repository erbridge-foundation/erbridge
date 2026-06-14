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
    if !accounts::account_exists(pool, target).await? {
        return Err(AppError::NotFound);
    }

    let mut tx = pool.begin().await?;
    let changed = accounts::set_server_admin(&mut tx, target, true).await?;
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
        .await?;
    }
    tx.commit().await?;
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
    if !accounts::account_exists(pool, target).await? {
        return Err(AppError::NotFound);
    }

    let mut tx = pool.begin().await?;

    // Lock + count the active admins *before* the flip. `count_server_admins_tx`
    // takes a `FOR UPDATE` lock on every active admin row, so concurrent revokes
    // serialise on that shared row set; locking before (not after) the flip
    // avoids a lock-ordering deadlock between two revokes of different targets.
    // A count <= 1 means the only active admin is the target itself, so revoking
    // it would leave zero — reject.
    let active_admins = accounts::count_server_admins_tx(&mut tx).await?;

    let changed = accounts::set_server_admin(&mut tx, target, false).await?;

    if !changed {
        // Target was not an admin — idempotent no-op. Nothing was changed, so
        // the rollback/commit are equivalent; commit to release the tx cleanly.
        tx.commit().await?;
        return Ok(());
    }

    if active_admins <= 1 {
        tx.rollback().await?;
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
    .await?;
    tx.commit().await?;
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
    let owning_account = characters::find_account_for_eve_character(pool, eve_character_id).await?;

    // Self-block guard: reject if the target character belongs to the actor's
    // own account. Writes nothing.
    if owning_account == Some(actor) {
        return Err(AppError::Conflict(ConflictKind::CannotBlockSelf));
    }

    let mut tx = pool.begin().await?;

    let inserted = blocks::insert_block(
        &mut tx,
        eve_character_id,
        character_name,
        corporation_name,
        reason,
        actor,
    )
    .await?;

    if !inserted {
        // Already blocked — idempotent no-op, no teardown, no audit.
        tx.commit().await?;
        return Ok(());
    }

    // Tear down the owning account (if any), mirroring soft-delete: clear EVE
    // tokens and delete all sessions, atomically with the block insert.
    if let Some(account_id) = owning_account {
        characters::clear_tokens_for_account(&mut tx, account_id).await?;
        sessions::delete_for_account_in_tx(&mut tx, account_id).await?;
    }

    audit::record_in_tx(
        &mut tx,
        Some(actor),
        None,
        AuditEvent::EveCharacterBlocked {
            eve_character_id,
            character_name: character_name.map(|n| n.to_string()),
            reason: reason.map(|r| r.to_string()),
        },
    )
    .await?;
    tx.commit().await?;
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
    let mut tx = pool.begin().await?;

    let character_name = match blocks::delete_block(&mut tx, eve_character_id).await? {
        Some(name) => name,
        None => {
            tx.rollback().await?;
            return Err(AppError::NotFound);
        }
    };

    audit::record_in_tx(
        &mut tx,
        Some(actor),
        None,
        AuditEvent::EveCharacterUnblocked {
            eve_character_id,
            character_name,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Re-exported so the handler maps a DTO from a service-owned type.
pub use crate::db::accounts::HardDeletePreview;

/// Returns the hard-delete blast-radius preview for `account_id`. `NotFound`
/// (404) when the account does not exist, so the admin UI never previews a ghost.
pub async fn hard_delete_preview(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<HardDeletePreview, AppError> {
    if !accounts::account_exists(pool, account_id).await? {
        return Err(AppError::NotFound);
    }
    Ok(accounts::hard_delete_preview(pool, account_id).await?)
}

/// Irreversibly hard-deletes `account_id` (`DELETE FROM account`), behind the
/// `AdminAccount` extractor at the handler. `NotFound` (404) for a missing
/// account. The last-server-admin guard runs INSIDE the transaction: if the
/// target is an active server admin and deleting it would leave no other active
/// admin, the request is refused with `CannotRemoveLastServerAdmin` (409) and
/// nothing is deleted. Emits `AccountHardDeleted` (actored by `actor`, carrying
/// the deleted account's `last_known_main_character_name` snapshot) in the same
/// transaction, before the delete, so the actor/snapshot resolve while the row
/// still exists. The FK graph enacts the blast radius (characters/sessions/keys
/// CASCADE away; maps/ACLs/audit/blocks SET NULL). Returns the preview counts.
pub async fn hard_delete_account(
    pool: &PgPool,
    actor: Uuid,
    account_id: Uuid,
) -> Result<HardDeletePreview, AppError> {
    let account = accounts::get_account(pool, account_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Snapshot the blast radius before the delete for the response + so the admin
    // sees what was removed even if they skipped the preview endpoint.
    let preview = accounts::hard_delete_preview(pool, account_id).await?;

    let mut tx = pool.begin().await?;

    // Last-admin guard, inside the tx, before the delete. `count_server_admins_tx`
    // FOR UPDATE-locks every active admin so concurrent admin removals serialise;
    // a count <= 1 with the target itself being that admin means deleting it would
    // leave zero. Mirrors `services::account::delete_account`.
    if account.is_server_admin {
        let admin_count = accounts::count_server_admins_tx(&mut tx).await?;
        if admin_count <= 1 {
            return Err(AppError::Conflict(
                ConflictKind::CannotRemoveLastServerAdmin,
            ));
        }
    }

    // Capture the snapshot name in-tx, then emit the audit BEFORE the delete so
    // the actor main + target snapshot resolve while rows still exist.
    let last_known_main_name = accounts::get_last_known_main_name(&mut tx, account_id).await?;
    audit::record_in_tx(
        &mut tx,
        Some(actor),
        None,
        AuditEvent::AccountHardDeleted {
            account_id,
            last_known_main_name,
        },
    )
    .await?;

    accounts::hard_delete(&mut tx, account_id).await?;
    tx.commit().await?;

    Ok(preview)
}

/// An account with its characters, as surfaced by the admin accounts list.
/// Re-exported from the db layer so the DTO maps from a service-owned type.
pub use crate::db::accounts::AccountWithCharacters as AdminAccountInfo;

/// All accounts (newest first) each with its characters, for the admin accounts
/// list.
pub async fn list_accounts(pool: &PgPool) -> Result<Vec<AdminAccountInfo>, AppError> {
    Ok(accounts::list_accounts_with_characters(pool).await?)
}

/// All block rows, newest first (pass-through for the admin block list).
pub async fn list_blocks(pool: &PgPool) -> Result<Vec<blocks::BlockedEveCharacter>, AppError> {
    Ok(blocks::list_blocks(pool).await?)
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
    let matches = characters::search_by_name(pool, q, limit).await?;

    // One query for the blocked subset, rather than one per result.
    let ids: Vec<i64> = matches.iter().map(|m| m.eve_character_id).collect();
    let blocked = blocks::blocked_set(pool, &ids).await?;

    let out = matches
        .into_iter()
        .map(|m| AdminCharacterSearchResult {
            portrait_url: esi::portrait_url(m.eve_character_id),
            already_blocked: blocked.contains(&m.eve_character_id),
            eve_character_id: m.eve_character_id,
            name: m.name,
            is_main: m.is_main,
            account_id: m.account_id,
        })
        .collect();
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

    // One query for the blocked subset, rather than one per result.
    let ids: Vec<i64> = results
        .characters
        .iter()
        .map(|c| c.eve_character_id)
        .collect();
    let blocked = blocks::blocked_set(pool, &ids).await?;

    let out = results
        .characters
        .into_iter()
        .map(|c| EsiCharacterSearchResult {
            portrait_url: esi::portrait_url(c.eve_character_id),
            already_blocked: blocked.contains(&c.eve_character_id),
            eve_character_id: c.eve_character_id,
            name: c.name,
        })
        .collect();

    Ok(EsiSearchOutcome::Available(out))
}

/// Audit-log pass-through forwarding every filter axis — the target-first axes
/// (`target_type` / `target_id` / `target_name`), the combined name search
/// (`q`), the `since` lower time bound and `before` keyset cursor / upper time
/// bound (together the time window) — plus a clamped `limit`.
#[allow(clippy::too_many_arguments)]
pub async fn list_audit_log(
    pool: &PgPool,
    event_type: Option<&str>,
    actor: Option<Uuid>,
    target_type: Option<&str>,
    target_id: Option<&str>,
    target_name: Option<&str>,
    q: Option<&str>,
    since: Option<DateTime<Utc>>,
    before: Option<DateTime<Utc>>,
    limit: Option<i64>,
) -> Result<Vec<AuditLogEntry>, AppError> {
    let limit = clamp_limit(limit);
    Ok(audit::list_audit_log(
        pool,
        event_type,
        actor,
        target_type,
        target_id,
        target_name,
        q,
        since,
        before,
        limit,
    )
    .await?)
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
        char_db::promote_if_no_main(&mut tx, account_id, char_id, eve_id, name)
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

    // ── hard delete + preview ───────────────────────────────────────────────

    async fn account_exists_row(pool: &PgPool, id: Uuid) -> bool {
        sqlx::query!(
            r#"SELECT EXISTS(SELECT 1 FROM account WHERE id = $1) AS "e!""#,
            id
        )
        .fetch_one(pool)
        .await
        .unwrap()
        .e
    }

    #[sqlx::test]
    async fn hard_delete_preview_counts_removed_and_unowned(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        let target = account_with_main(&pool, 2, "Target", false).await;
        // One session, one api key (removed); one map, one acl (unowned).
        sqlx::query!(
            "INSERT INTO session (session_id, account_id, expires_at)
             VALUES ('s', $1, now() + interval '7 days')",
            target
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO api_key (scope, account_id, name, key_hash)
             VALUES ('account', $1, 'k', 'h')",
            target
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO map (name, slug, owner_account_id) VALUES ('M', 'm', $1)",
            target
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO acl (name, owner_account_id) VALUES ('A', $1)",
            target
        )
        .execute(&pool)
        .await
        .unwrap();

        let preview = hard_delete_preview(&pool, target).await.unwrap();
        assert_eq!(preview.characters, 1);
        assert_eq!(preview.sessions, 1);
        assert_eq!(preview.api_keys, 1);
        assert_eq!(preview.owned_maps, 1);
        assert_eq!(preview.owned_acls, 1);
        let _ = admin;
    }

    #[sqlx::test]
    async fn hard_delete_preview_404_for_missing(pool: PgPool) {
        let err = hard_delete_preview(&pool, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound));
    }

    #[sqlx::test]
    async fn hard_delete_cascades_private_and_set_nulls_co_owned(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        let target = account_with_main(&pool, 2, "Target", false).await;
        sqlx::query!(
            "INSERT INTO session (session_id, account_id, expires_at)
             VALUES ('s', $1, now() + interval '7 days')",
            target
        )
        .execute(&pool)
        .await
        .unwrap();
        let map_id = sqlx::query!(
            "INSERT INTO map (name, slug, owner_account_id) VALUES ('M', 'm', $1) RETURNING id",
            target
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .id;
        let acl_id = sqlx::query!(
            "INSERT INTO acl (name, owner_account_id) VALUES ('A', $1) RETURNING id",
            target
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .id;
        // An audit row actored by the target, to assert SET NULL preserves it.
        audit::record_in_tx(
            &mut pool.begin().await.unwrap(),
            None,
            None,
            AuditEvent::AccountReactivated { account_id: target },
        )
        .await
        .ok();

        hard_delete_account(&pool, admin, target).await.unwrap();

        // Account + its private rows gone.
        assert!(!account_exists_row(&pool, target).await);
        let chars =
            sqlx::query!("SELECT COUNT(*) AS \"c!\" FROM eve_character WHERE eve_character_id = 2")
                .fetch_one(&pool)
                .await
                .unwrap()
                .c;
        assert_eq!(chars, 0, "character cascaded away");
        let sessions =
            sqlx::query!("SELECT COUNT(*) AS \"c!\" FROM session WHERE session_id = 's'")
                .fetch_one(&pool)
                .await
                .unwrap()
                .c;
        assert_eq!(sessions, 0, "session cascaded away");

        // Co-owned rows survive, unowned.
        let map_owner = sqlx::query!("SELECT owner_account_id FROM map WHERE id = $1", map_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .owner_account_id;
        assert_eq!(map_owner, None, "map survives, unowned");
        let acl_owner = sqlx::query!("SELECT owner_account_id FROM acl WHERE id = $1", acl_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .owner_account_id;
        assert_eq!(acl_owner, None, "acl survives, unowned");

        // The hard-delete is audited.
        assert_eq!(audit_count(&pool, "account_hard_deleted").await, 1);
    }

    #[sqlx::test]
    async fn hard_delete_last_admin_guard_rejects(pool: PgPool) {
        // A lone admin cannot be hard-deleted (would leave zero admins).
        let only = account_with_main(&pool, 1, "Only", true).await;
        let err = hard_delete_account(&pool, only, only).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotRemoveLastServerAdmin)
        ));
        assert!(account_exists_row(&pool, only).await, "nothing deleted");
        assert_eq!(audit_count(&pool, "account_hard_deleted").await, 0);
    }

    #[sqlx::test]
    async fn hard_delete_admin_allowed_when_another_admin_exists(pool: PgPool) {
        let a = account_with_main(&pool, 1, "A", true).await;
        let b = account_with_main(&pool, 2, "B", true).await;
        hard_delete_account(&pool, a, b).await.unwrap();
        assert!(!account_exists_row(&pool, b).await);
    }

    #[sqlx::test]
    async fn hard_delete_404_for_missing(pool: PgPool) {
        let admin = account_with_main(&pool, 1, "Admin", true).await;
        let err = hard_delete_account(&pool, admin, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound));
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

    use crate::esi::jwks::JwksCache;
    use crate::esi::test_support::{EsiClaims, TestKeypair, jwks_json, test_keypair};

    /// An RS256-signed access token (kid `kid-1`) carrying the given owner hash
    /// for character 42, for mocking the SSO refresh endpoint. Pair with
    /// [`jwks_cache`] (built from the same keypair) so verification succeeds.
    fn refresh_access_jwt(kp: &TestKeypair, owner: &str) -> String {
        kp.sign(&EsiClaims::valid(42, "Self", owner))
    }

    /// A JWKS cache holding only `kp`'s public key, no network refetch.
    fn jwks_cache(kp: &TestKeypair) -> JwksCache {
        JwksCache::from_keys_for_test(
            http(),
            "http://unused",
            crate::esi::jwks::decode_keys_for_test(jwks_json(&[kp]).as_bytes()),
        )
    }

    /// An ESI context pointing search/resolve at `esi_base` and token refresh at
    /// `token_endpoint`. Uses the all-zero test key matching `account_with_main`.
    fn ctx<'a>(
        http: &'a ClientWithMiddleware,
        jwks: &'a JwksCache,
        esi_base: &'a str,
        token_endpoint: &'a str,
    ) -> EsiSearchContext<'a> {
        EsiSearchContext {
            http,
            jwks,
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
        char_db::promote_if_no_main(&mut tx, account_id, char_id, eve_id, "Expired")
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
        let kp = test_keypair("kid-1");
        let jwks = jwks_cache(&kp);
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, &jwks, "http://unused", "http://unused"),
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
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 555, "name": "Blocked Pilot", "category": "character" },
                { "id": 777, "name": "Free Pilot", "category": "character" }
            ])))
            .mount(&server)
            .await;

        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(
                &client,
                &jwks_cache(&test_keypair("kid-1")),
                &server.uri(),
                "http://unused",
            ),
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
            ctx(
                &client,
                &jwks_cache(&test_keypair("kid-1")),
                &server.uri(),
                "http://unused",
            ),
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
            ctx(
                &client,
                &jwks_cache(&test_keypair("kid-1")),
                &server.uri(),
                "http://unused",
            ),
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
        let kp = test_keypair("kid-1");
        let jwks = jwks_cache(&kp);

        let server = MockServer::start().await;
        // Token endpoint returns a fresh token set. The access token must be a
        // JWT the refresh path can verify against the JWKS and whose `owner`
        // claim it then reads.
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": refresh_access_jwt(&kp, "owner-hash"),
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
        Mock::given(method("POST"))
            .and(path("/universe/names/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 42, "name": "Self", "category": "character" }
            ])))
            .mount(&server)
            .await;

        let token_endpoint = format!("{}/oauth/token", server.uri());
        let client = http();
        let outcome = esi_search_characters(
            &pool,
            ctx(&client, &jwks, &server.uri(), &token_endpoint),
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
            ctx(
                &client,
                &jwks_cache(&test_keypair("kid-1")),
                &server.uri(),
                &token_endpoint,
            ),
            admin,
            "sel",
            None,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, EsiSearchOutcome::Unavailable));
    }
}
