use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub struct Account {
    pub id: Uuid,
    pub status: String,
    pub delete_requested_at: Option<DateTime<Utc>>,
    pub is_server_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_account(pool: &PgPool) -> Result<Uuid> {
    let row = sqlx::query!("INSERT INTO account DEFAULT VALUES RETURNING id")
        .fetch_one(pool)
        .await
        .context("failed to create account")?;
    Ok(row.id)
}

pub async fn get_account(pool: &PgPool, id: Uuid) -> Result<Option<Account>> {
    let row = sqlx::query!(
        "SELECT id, status, delete_requested_at, is_server_admin, created_at, updated_at
         FROM account WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await
    .context("failed to fetch account")?;

    Ok(row.map(|r| Account {
        id: r.id,
        status: r.status,
        delete_requested_at: r.delete_requested_at,
        is_server_admin: r.is_server_admin,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }))
}

/// Reactivates a soft-deleted account. Returns `true` if a row was actually
/// flipped from `soft_deleted` to `active`, `false` if the account was already
/// active (in which case the caller knows reactivation did not just happen and
/// SHOULD NOT emit an `account_reactivated` audit event).
pub async fn reactivate_if_soft_deleted(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<bool> {
    let result = sqlx::query!(
        "UPDATE account SET status = 'active', delete_requested_at = NULL
         WHERE id = $1 AND status = 'soft_deleted'",
        id
    )
    .execute(&mut **tx)
    .await
    .context("failed to reactivate account")?;
    Ok(result.rows_affected() > 0)
}

pub async fn soft_delete(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<()> {
    sqlx::query!(
        "UPDATE account SET status = 'soft_deleted', delete_requested_at = now()
         WHERE id = $1",
        id
    )
    .execute(&mut **tx)
    .await
    .context("failed to soft delete account")?;
    Ok(())
}

/// Stamps `last_login = now()` on an account. Called within the SSO callback
/// transaction; the daily sweep's 7-day idle waterfall reads this clock.
pub async fn set_last_login(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<()> {
    sqlx::query!("UPDATE account SET last_login = now() WHERE id = $1", id)
        .execute(&mut **tx)
        .await
        .context("failed to set account last_login")?;
    Ok(())
}

/// Account ids whose `last_login` is older than `days` days. A NULL `last_login`
/// is excluded (treated as "not yet observed") so legacy accounts are not
/// mass-expired on the first sweep run. Backs the sweep's idle waterfall.
pub async fn list_idle_accounts(pool: &PgPool, days: i64) -> Result<Vec<Uuid>> {
    let rows = sqlx::query!(
        "SELECT id FROM account
         WHERE last_login IS NOT NULL
           AND last_login < now() - make_interval(days => $1::int)",
        days as i32
    )
    .fetch_all(pool)
    .await
    .context("failed to list idle accounts")?;
    Ok(rows.into_iter().map(|r| r.id).collect())
}

/// What happened during `resolve_or_create`. The audit-emit code uses this to
/// decide which events to record (account_registered, orphan_character_claimed,
/// server_admin_granted{bootstrap}).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionOutcome {
    /// `add_character_account_id` was supplied — used the session's account.
    AddCharacterMode,
    /// A character row already exists for this `eve_character_id` and is
    /// already bound to an account.
    ExistingAccount,
    /// A character row already exists for this `eve_character_id` but has
    /// `account_id IS NULL` — caller will claim the orphan in a follow-up step.
    OrphanCharacterExists,
    /// No matching character row exists; a brand-new account was created.
    /// `bootstrapped_admin` is `true` iff this is the first-ever account, so
    /// it was auto-promoted to server admin.
    NewAccount { bootstrapped_admin: bool },
}

/// Returns the account that already owns this `eve_character_id` if present, the
/// session's `add_character_account_id` when in add-character mode, or creates a
/// new account row otherwise. Also returns a `ResolutionOutcome` describing which
/// path was taken so callers can attribute the SSO-callback transaction's audit
/// events correctly.
pub async fn resolve_or_create(
    tx: &mut Transaction<'_, Postgres>,
    add_character_account_id: Option<Uuid>,
    eve_character_id: i64,
) -> Result<(Uuid, ResolutionOutcome)> {
    if let Some(account_id) = add_character_account_id {
        return Ok((account_id, ResolutionOutcome::AddCharacterMode));
    }

    // Check if a character with this eve_character_id already has an account.
    let existing = sqlx::query!(
        "SELECT account_id FROM eve_character WHERE eve_character_id = $1",
        eve_character_id
    )
    .fetch_optional(&mut **tx)
    .await
    .context("failed to look up existing character")?;

    if let Some(row) = existing {
        if let Some(account_id) = row.account_id {
            return Ok((account_id, ResolutionOutcome::ExistingAccount));
        }
        // Orphan character — the caller's follow-up upsert will set account_id.
        let new_account = create_account_with_bootstrap(tx).await?;
        return Ok((new_account.0, ResolutionOutcome::OrphanCharacterExists));
    }

    // No row at all — create a fresh account and let the caller insert a fresh
    // eve_character row.
    let (account_id, bootstrapped_admin) = create_account_with_bootstrap(tx).await?;
    Ok((
        account_id,
        ResolutionOutcome::NewAccount { bootstrapped_admin },
    ))
}

/// Inserts a new `account` row, auto-promoting it to server admin iff no
/// other account row exists. Returns `(id, bootstrapped_admin)`.
async fn create_account_with_bootstrap(tx: &mut Transaction<'_, Postgres>) -> Result<(Uuid, bool)> {
    let row = sqlx::query!(
        "INSERT INTO account (is_server_admin)
         VALUES (NOT EXISTS (SELECT 1 FROM account))
         RETURNING id, is_server_admin"
    )
    .fetch_one(&mut **tx)
    .await
    .context("failed to create account")?;
    Ok((row.id, row.is_server_admin))
}

pub async fn count_server_admins(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query!(
        "SELECT COUNT(*) AS \"count!\" FROM account
         WHERE is_server_admin = TRUE AND status = 'active'"
    )
    .fetch_one(pool)
    .await
    .context("failed to count server admins")?;
    Ok(row.count)
}

/// The same active-admin count as `count_server_admins`, but participating in
/// the caller's transaction. The revoke-admin last-admin guard runs this
/// *inside* the revoke transaction so the count is consistent with the pending
/// `UPDATE` (the pool-based variant stays for the soft-delete guard, which
/// reads outside any transaction).
pub async fn count_server_admins_tx(tx: &mut Transaction<'_, Postgres>) -> Result<i64> {
    let row = sqlx::query!(
        "SELECT COUNT(*) AS \"count!\" FROM account
         WHERE is_server_admin = TRUE AND status = 'active'"
    )
    .fetch_one(&mut **tx)
    .await
    .context("failed to count server admins (tx)")?;
    Ok(row.count)
}

/// Sets or clears `is_server_admin` on an account. Returns `true` if a row was
/// actually changed (the flag flipped), `false` if the account already had the
/// requested value — letting the service skip the audit emission on an
/// idempotent no-op grant/revoke.
pub async fn set_server_admin(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    value: bool,
) -> Result<bool> {
    let result = sqlx::query!(
        "UPDATE account SET is_server_admin = $2, updated_at = now()
         WHERE id = $1 AND is_server_admin <> $2",
        account_id,
        value,
    )
    .execute(&mut **tx)
    .await
    .context("failed to set server admin flag")?;
    Ok(result.rows_affected() > 0)
}

/// Whether an account row exists for `account_id`. Used by grant/revoke to
/// return 404 for a non-existent target without fetching the whole row.
pub async fn account_exists(pool: &PgPool, account_id: Uuid) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT EXISTS (SELECT 1 FROM account WHERE id = $1) AS "exists!""#,
        account_id
    )
    .fetch_one(pool)
    .await
    .context("failed to check account existence")?;
    Ok(row.exists)
}

/// Every account, newest first. Backs the admin accounts list; the service
/// layer assembles each account's characters separately.
pub async fn list_accounts_admin(pool: &PgPool) -> Result<Vec<Account>> {
    let rows = sqlx::query!(
        "SELECT id, status, delete_requested_at, is_server_admin, created_at, updated_at
         FROM account ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
    .context("failed to list accounts")?;

    Ok(rows
        .into_iter()
        .map(|r| Account {
            id: r.id,
            status: r.status,
            delete_requested_at: r.delete_requested_at,
            is_server_admin: r.is_server_admin,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect())
}

/// An account together with a lightweight view of its characters
/// (`eve_character_id`, `name`, `is_main`, `token_status`) — what the admin
/// accounts list needs to identify accounts by pilot and flag transferred /
/// expired characters, without the credential columns.
pub struct AccountWithCharacters {
    pub account: Account,
    pub characters: Vec<(i64, String, bool, String)>,
}

/// Every account (newest first) with its characters, assembled in one query
/// (LEFT JOIN so character-less accounts still appear). Characters within an
/// account are ordered main-first then by name for a stable display.
pub async fn list_accounts_with_characters(pool: &PgPool) -> Result<Vec<AccountWithCharacters>> {
    let rows = sqlx::query!(
        r#"
        SELECT a.id, a.status, a.delete_requested_at, a.is_server_admin,
               a.created_at, a.updated_at,
               c.eve_character_id AS "eve_character_id?",
               c.name AS "character_name?",
               c.is_main AS "is_main?",
               c.token_status AS "token_status?"
        FROM account a
        LEFT JOIN eve_character c ON c.account_id = a.id
        ORDER BY a.created_at DESC, c.is_main DESC, c.name ASC
        "#
    )
    .fetch_all(pool)
    .await
    .context("failed to list accounts with characters")?;

    // Group consecutive rows by account id (the ORDER BY keeps each account's
    // rows contiguous).
    let mut out: Vec<AccountWithCharacters> = Vec::new();
    for r in rows {
        let needs_new = out.last().map(|e| e.account.id != r.id).unwrap_or(true);
        if needs_new {
            out.push(AccountWithCharacters {
                account: Account {
                    id: r.id,
                    status: r.status,
                    delete_requested_at: r.delete_requested_at,
                    is_server_admin: r.is_server_admin,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                },
                characters: Vec::new(),
            });
        }
        if let (Some(eve_id), Some(name), Some(is_main), Some(token_status)) = (
            r.eve_character_id,
            r.character_name,
            r.is_main,
            r.token_status,
        ) {
            #[allow(clippy::unwrap_used)]
            out.last_mut()
                .unwrap()
                .characters
                .push((eve_id, name, is_main, token_status));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    #[sqlx::test]
    async fn create_account_returns_uuid(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        assert!(!id.is_nil());
    }

    #[sqlx::test]
    async fn get_account_returns_defaults(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        let account = get_account(&pool, id).await.unwrap().unwrap();
        assert_eq!(account.status, "active");
        assert!(account.delete_requested_at.is_none());
        assert!(!account.is_server_admin);
    }

    #[sqlx::test]
    async fn get_account_returns_none_for_missing(pool: PgPool) {
        let id = Uuid::new_v4();
        let result = get_account(&pool, id).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn reactivate_if_soft_deleted_restores_active(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        sqlx::query!(
            "UPDATE account SET status = 'soft_deleted', delete_requested_at = now() WHERE id = $1",
            id
        )
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        reactivate_if_soft_deleted(&mut tx, id).await.unwrap();
        tx.commit().await.unwrap();

        let account = get_account(&pool, id).await.unwrap().unwrap();
        assert_eq!(account.status, "active");
        assert!(account.delete_requested_at.is_none());
    }

    #[sqlx::test]
    async fn reactivate_if_soft_deleted_noop_on_active(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        reactivate_if_soft_deleted(&mut tx, id).await.unwrap();
        tx.commit().await.unwrap();

        let account = get_account(&pool, id).await.unwrap().unwrap();
        assert_eq!(account.status, "active");
    }

    #[sqlx::test]
    async fn resolve_or_create_promotes_first_account_to_server_admin(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (id, outcome) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(
            outcome,
            ResolutionOutcome::NewAccount {
                bootstrapped_admin: true
            }
        );
        let account = get_account(&pool, id).await.unwrap().unwrap();
        assert!(account.is_server_admin);
    }

    #[sqlx::test]
    async fn resolve_or_create_does_not_promote_subsequent_accounts(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (_first, _) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        let (second, outcome) = resolve_or_create(&mut tx, None, 1002).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(
            outcome,
            ResolutionOutcome::NewAccount {
                bootstrapped_admin: false
            }
        );
        let account = get_account(&pool, second).await.unwrap().unwrap();
        assert!(!account.is_server_admin);
    }

    #[sqlx::test]
    async fn resolve_or_create_skips_bootstrap_when_soft_deleted_admin_exists(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (first, _) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        soft_delete(&mut tx, first).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        let (second, _) = resolve_or_create(&mut tx, None, 1002).await.unwrap();
        tx.commit().await.unwrap();

        let account = get_account(&pool, second).await.unwrap().unwrap();
        assert!(!account.is_server_admin);
    }

    #[sqlx::test]
    async fn resolve_or_create_returns_existing_account_for_known_character(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (first, _) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        // Insert a character row binding this eve_character_id to the account.
        sqlx::query!(
            "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name)
             VALUES ($1, $2, 'Test', 1_000_001, 'Test Corp')",
            first,
            1001_i64,
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        let (resolved, outcome) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(resolved, first);
        assert_eq!(outcome, ResolutionOutcome::ExistingAccount);
    }

    #[sqlx::test]
    async fn resolve_or_create_signals_orphan_character_exists(pool: PgPool) {
        // Insert an orphan character (account_id IS NULL).
        sqlx::query!(
            "INSERT INTO eve_character (eve_character_id, name, corporation_id, corporation_name)
             VALUES ($1, 'Orphan', 1_000_001, 'Test Corp')",
            1001_i64,
        )
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let (_account, outcome) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(outcome, ResolutionOutcome::OrphanCharacterExists);
    }

    #[sqlx::test]
    async fn resolve_or_create_signals_add_character_mode(pool: PgPool) {
        let existing = create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let (resolved, outcome) = resolve_or_create(&mut tx, Some(existing), 1001)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(resolved, existing);
        assert_eq!(outcome, ResolutionOutcome::AddCharacterMode);
    }

    #[sqlx::test]
    async fn reactivate_if_soft_deleted_returns_true_when_actually_reactivated(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        sqlx::query!(
            "UPDATE account SET status = 'soft_deleted', delete_requested_at = now() WHERE id = $1",
            id
        )
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let fired = reactivate_if_soft_deleted(&mut tx, id).await.unwrap();
        tx.commit().await.unwrap();

        assert!(fired);
    }

    #[sqlx::test]
    async fn reactivate_if_soft_deleted_returns_false_when_already_active(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let fired = reactivate_if_soft_deleted(&mut tx, id).await.unwrap();
        tx.commit().await.unwrap();

        assert!(!fired);
    }

    #[sqlx::test]
    async fn count_server_admins_counts_only_active_admins(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (first, _) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        let (_second, _) = resolve_or_create(&mut tx, None, 1002).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(count_server_admins(&pool).await.unwrap(), 1);

        let mut tx = pool.begin().await.unwrap();
        soft_delete(&mut tx, first).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(count_server_admins(&pool).await.unwrap(), 0);
    }

    #[sqlx::test]
    async fn soft_delete_sets_status(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        soft_delete(&mut tx, id).await.unwrap();
        tx.commit().await.unwrap();
        let account = get_account(&pool, id).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert!(account.delete_requested_at.is_some());
    }

    #[sqlx::test]
    async fn set_server_admin_flips_flag_and_reports_change(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        assert!(
            !get_account(&pool, id)
                .await
                .unwrap()
                .unwrap()
                .is_server_admin
        );

        let mut tx = pool.begin().await.unwrap();
        let changed = set_server_admin(&mut tx, id, true).await.unwrap();
        tx.commit().await.unwrap();
        assert!(changed, "flipping false->true is a change");
        assert!(
            get_account(&pool, id)
                .await
                .unwrap()
                .unwrap()
                .is_server_admin
        );

        // Idempotent: setting the same value again changes no row.
        let mut tx = pool.begin().await.unwrap();
        let changed = set_server_admin(&mut tx, id, true).await.unwrap();
        tx.commit().await.unwrap();
        assert!(!changed, "setting true->true is a no-op");

        // And flipping back reports a change.
        let mut tx = pool.begin().await.unwrap();
        let changed = set_server_admin(&mut tx, id, false).await.unwrap();
        tx.commit().await.unwrap();
        assert!(changed);
        assert!(
            !get_account(&pool, id)
                .await
                .unwrap()
                .unwrap()
                .is_server_admin
        );
    }

    #[sqlx::test]
    async fn account_exists_true_and_false(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        assert!(account_exists(&pool, id).await.unwrap());
        assert!(!account_exists(&pool, Uuid::new_v4()).await.unwrap());
    }

    #[sqlx::test]
    async fn count_server_admins_tx_matches_pool_variant(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (_first, _) = resolve_or_create(&mut tx, None, 1001).await.unwrap();
        let (_second, _) = resolve_or_create(&mut tx, None, 1002).await.unwrap();
        // Within the same tx, only the bootstrapped first account is admin.
        assert_eq!(count_server_admins_tx(&mut tx).await.unwrap(), 1);
        tx.commit().await.unwrap();
        assert_eq!(count_server_admins(&pool).await.unwrap(), 1);
    }

    #[sqlx::test]
    async fn list_accounts_admin_is_newest_first(pool: PgPool) {
        let first = create_account(&pool).await.unwrap();
        let second = create_account(&pool).await.unwrap();
        let accounts = list_accounts_admin(&pool).await.unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].id, second, "newest (created_at DESC) first");
        assert_eq!(accounts[1].id, first);
    }

    #[sqlx::test]
    async fn set_last_login_stamps_now(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        // New accounts have NULL last_login.
        assert!(list_idle_accounts(&pool, 0).await.unwrap().is_empty());

        let mut tx = pool.begin().await.unwrap();
        set_last_login(&mut tx, id).await.unwrap();
        tx.commit().await.unwrap();

        let r = sqlx::query!("SELECT last_login FROM account WHERE id = $1", id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(r.last_login.is_some());
    }

    #[sqlx::test]
    async fn list_idle_accounts_excludes_null_and_recent(pool: PgPool) {
        // Idle: last_login well in the past.
        let idle = create_account(&pool).await.unwrap();
        sqlx::query!(
            "UPDATE account SET last_login = now() - interval '30 days' WHERE id = $1",
            idle
        )
        .execute(&pool)
        .await
        .unwrap();

        // Recent: logged in just now.
        let recent = create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        set_last_login(&mut tx, recent).await.unwrap();
        tx.commit().await.unwrap();

        // Never-logged-in: NULL last_login (excluded).
        let _never = create_account(&pool).await.unwrap();

        let idle_ids = list_idle_accounts(&pool, 7).await.unwrap();
        assert_eq!(idle_ids, vec![idle]);
    }
}
