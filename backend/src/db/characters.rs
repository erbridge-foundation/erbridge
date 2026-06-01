use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::handlers::crypto;

pub struct Character {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub eve_character_id: i64,
    pub name: String,
    pub corporation_id: i64,
    pub corporation_name: String,
    pub alliance_id: Option<i64>,
    pub alliance_name: Option<String>,
    pub is_main: bool,
    pub is_online: Option<bool>,
    pub esi_client_id: Option<String>,
    pub encrypted_refresh_token: Option<Vec<u8>>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub scopes: Vec<String>,
    pub owner_hash: Option<String>,
    pub token_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_tokens(
    tx: &mut Transaction<'_, Postgres>,
    resolved_account_id: Uuid,
    eve_character_id: i64,
    name: &str,
    corporation_id: i64,
    corporation_name: &str,
    alliance_id: Option<i64>,
    alliance_name: Option<&str>,
    esi_client_id: &str,
    access_token_plaintext: &str,
    refresh_token_plaintext: &str,
    access_token_expires_at: DateTime<Utc>,
    scopes: &[String],
    owner_hash: &str,
    encryption_key: &[u8],
) -> Result<Uuid> {
    let encrypted_access = crypto::encrypt_token(access_token_plaintext, encryption_key)
        .context("failed to encrypt access token")?;
    let encrypted_refresh = crypto::encrypt_token(refresh_token_plaintext, encryption_key)
        .context("failed to encrypt refresh token")?;

    let row = sqlx::query!(
        r#"
        INSERT INTO eve_character (
            account_id, eve_character_id, name, corporation_id, corporation_name,
            alliance_id, alliance_name, esi_client_id, encrypted_access_token,
            encrypted_refresh_token, access_token_expires_at, scopes,
            owner_hash, token_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'valid')
        ON CONFLICT (eve_character_id) DO UPDATE SET
            account_id = CASE
                WHEN eve_character.account_id IS NULL THEN excluded.account_id
                WHEN eve_character.account_id = excluded.account_id THEN excluded.account_id
                ELSE eve_character.account_id
            END,
            name = excluded.name,
            corporation_id = excluded.corporation_id,
            corporation_name = excluded.corporation_name,
            alliance_id = excluded.alliance_id,
            alliance_name = excluded.alliance_name,
            esi_client_id = excluded.esi_client_id,
            encrypted_access_token = excluded.encrypted_access_token,
            encrypted_refresh_token = excluded.encrypted_refresh_token,
            access_token_expires_at = excluded.access_token_expires_at,
            scopes = excluded.scopes,
            owner_hash = excluded.owner_hash,
            -- A successful callback always presents a current owner hash, so it
            -- restores the character to a healthy state, self-healing a prior
            -- token_expired / owner_mismatch flag (see character-token-lifecycle).
            token_status = 'valid',
            updated_at = now()
        RETURNING id
        "#,
        resolved_account_id,
        eve_character_id,
        name,
        corporation_id,
        corporation_name,
        alliance_id,
        alliance_name,
        esi_client_id,
        encrypted_access.as_slice(),
        encrypted_refresh.as_slice(),
        access_token_expires_at,
        scopes,
        owner_hash,
    )
    .fetch_one(&mut **tx)
    .await
    .context("failed to upsert character tokens")?;

    Ok(row.id)
}

pub async fn promote_if_no_main(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    just_written_character_id: Uuid,
) -> Result<bool> {
    let result = sqlx::query!(
        r#"
        UPDATE eve_character SET is_main = TRUE
        WHERE id = $1
          AND NOT EXISTS (
              SELECT 1 FROM eve_character
              WHERE account_id = $2 AND is_main = TRUE
          )
        "#,
        just_written_character_id,
        account_id,
    )
    .execute(&mut **tx)
    .await
    .context("failed to promote character to main")?;

    Ok(result.rows_affected() > 0)
}

pub async fn create_orphan(
    pool: &PgPool,
    eve_character_id: i64,
    name: &str,
    corporation_id: i64,
    corporation_name: &str,
    alliance_id: Option<i64>,
    alliance_name: Option<&str>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO eve_character (
            eve_character_id, name, corporation_id, corporation_name,
            alliance_id, alliance_name
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
        eve_character_id,
        name,
        corporation_id,
        corporation_name,
        alliance_id,
        alliance_name,
    )
    .fetch_one(pool)
    .await
    .context("failed to create orphan character")?;

    Ok(row.id)
}

pub async fn list_for_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<Character>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, account_id, eve_character_id, name,
               corporation_id, corporation_name, alliance_id, alliance_name,
               is_main, is_online, esi_client_id, encrypted_refresh_token,
               access_token_expires_at, scopes, owner_hash, token_status,
               created_at, updated_at
        FROM eve_character
        WHERE account_id = $1
        ORDER BY created_at ASC
        "#,
        account_id
    )
    .fetch_all(pool)
    .await
    .context("failed to list characters for account")?;

    Ok(rows
        .into_iter()
        .map(|r| Character {
            id: r.id,
            account_id: r.account_id,
            eve_character_id: r.eve_character_id,
            name: r.name,
            corporation_id: r.corporation_id,
            corporation_name: r.corporation_name,
            alliance_id: r.alliance_id,
            alliance_name: r.alliance_name,
            is_main: r.is_main,
            is_online: r.is_online,
            esi_client_id: r.esi_client_id,
            encrypted_refresh_token: r.encrypted_refresh_token,
            access_token_expires_at: r.access_token_expires_at,
            scopes: r.scopes,
            owner_hash: r.owner_hash,
            token_status: r.token_status,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect())
}

pub async fn delete_character(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query!("DELETE FROM eve_character WHERE id = $1", id)
        .execute(pool)
        .await
        .context("failed to delete character")?;
    Ok(result.rows_affected() > 0)
}

/// Transactional variant of [`delete_character`] for callers that need to
/// commit the delete alongside an audit emission in one transaction.
pub async fn delete_character_in_tx(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<bool> {
    let result = sqlx::query!("DELETE FROM eve_character WHERE id = $1", id)
        .execute(&mut **tx)
        .await
        .context("failed to delete character")?;
    Ok(result.rows_affected() > 0)
}

pub async fn set_main(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    character_id: Uuid,
) -> Result<()> {
    sqlx::query!(
        "UPDATE eve_character SET is_main = FALSE WHERE account_id = $1",
        account_id
    )
    .execute(&mut **tx)
    .await
    .context("failed to clear existing main")?;

    sqlx::query!(
        "UPDATE eve_character SET is_main = TRUE WHERE id = $1 AND account_id = $2",
        character_id,
        account_id,
    )
    .execute(&mut **tx)
    .await
    .context("failed to set new main")?;

    Ok(())
}

pub async fn count_for_account(pool: &PgPool, account_id: Uuid) -> Result<i64> {
    let row = sqlx::query!(
        "SELECT COUNT(*) as count FROM eve_character WHERE account_id = $1",
        account_id
    )
    .fetch_one(pool)
    .await
    .context("failed to count characters for account")?;
    Ok(row.count.unwrap_or(0))
}

pub async fn clear_tokens_for_account(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE eve_character
        SET encrypted_access_token = NULL,
            encrypted_refresh_token = NULL,
            access_token_expires_at = NULL,
            scopes = '{}',
            updated_at = now()
        WHERE account_id = $1
        "#,
        account_id
    )
    .execute(&mut **tx)
    .await
    .context("failed to clear character tokens for account")?;
    Ok(())
}

pub async fn is_main(pool: &PgPool, id: Uuid) -> Result<Option<(Uuid, bool)>> {
    let row = sqlx::query!(
        "SELECT account_id, is_main FROM eve_character WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await
    .context("failed to fetch character main status")?;

    Ok(row.and_then(|r| r.account_id.map(|acc_id| (acc_id, r.is_main))))
}

/// Looks up the `(account_id, eve_character_id, is_main)` for an internal
/// character UUID. Returns `None` when no row exists or the row is an orphan
/// (`account_id IS NULL`). Used by audit-emitting services that need the EVE
/// ID alongside the ownership check.
pub async fn lookup_for_account(pool: &PgPool, id: Uuid) -> Result<Option<(Uuid, i64, bool)>> {
    let row = sqlx::query!(
        "SELECT account_id, eve_character_id, is_main
         FROM eve_character WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await
    .context("failed to look up character for account")?;

    Ok(row.and_then(|r| {
        r.account_id
            .map(|acc_id| (acc_id, r.eve_character_id, r.is_main))
    }))
}

/// Returns the `account_id` binding of an existing `eve_character` row keyed by
/// `eve_character_id`. The outer `Option` discriminates "no row exists" vs.
/// "row exists"; the inner `Option<Uuid>` discriminates orphan (NULL account_id)
/// vs. bound. Used by the SSO callback to decide whether an add-character flow
/// is claiming an orphan or adding a fresh character.
pub async fn find_account_id_for_eve_character(
    tx: &mut Transaction<'_, Postgres>,
    eve_character_id: i64,
) -> Result<Option<Option<Uuid>>> {
    let row = sqlx::query!(
        "SELECT account_id FROM eve_character WHERE eve_character_id = $1",
        eve_character_id
    )
    .fetch_optional(&mut **tx)
    .await
    .context("failed to look up eve_character account_id")?;

    Ok(row.map(|r| r.account_id))
}

/// A character matched by the admin name search, carrying its owning account so
/// the grant UI can resolve "promote the account that owns *Pilot X*".
pub struct CharacterSearchResult {
    pub eve_character_id: i64,
    pub name: String,
    pub is_main: bool,
    pub account_id: Option<Uuid>,
}

/// Case-insensitive substring search on character name, capped at `limit` rows
/// (newest-bound first by name for stable ordering). `fragment` binds as a
/// parameter — no SQL injection surface. LIKE metacharacters (`%`, `_`, `\`) in
/// the fragment are escaped so they match literally rather than as wildcards,
/// so a search for "50%" finds a pilot literally named with a percent sign and
/// never errors or matches everything.
pub async fn search_by_name(
    pool: &PgPool,
    fragment: &str,
    limit: i64,
) -> Result<Vec<CharacterSearchResult>> {
    // Escape LIKE wildcards, then wrap in %...% for a substring match. The
    // backslash is the default ESCAPE character in Postgres ILIKE.
    let escaped = fragment
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{escaped}%");

    let rows = sqlx::query!(
        r#"
        SELECT eve_character_id, name, is_main, account_id
        FROM eve_character
        WHERE name ILIKE $1
        ORDER BY name ASC
        LIMIT $2
        "#,
        pattern,
        limit,
    )
    .fetch_all(pool)
    .await
    .context("failed to search characters by name")?;

    Ok(rows
        .into_iter()
        .map(|r| CharacterSearchResult {
            eve_character_id: r.eve_character_id,
            name: r.name,
            is_main: r.is_main,
            account_id: r.account_id,
        })
        .collect())
}

/// The account that owns this `eve_character_id`, or `None` when the character
/// is an orphan (`account_id IS NULL`) or has never been seen (no row). Used by
/// the block service to decide whether a block must tear down an owning account.
/// Distinct from `find_account_id_for_eve_character` (tx-scoped, two-level
/// Option) — this is the pool-based "give me the owning account or nothing".
pub async fn find_account_for_eve_character(
    pool: &PgPool,
    eve_character_id: i64,
) -> Result<Option<Uuid>> {
    let row = sqlx::query!(
        "SELECT account_id FROM eve_character WHERE eve_character_id = $1",
        eve_character_id
    )
    .fetch_optional(pool)
    .await
    .context("failed to find account for eve_character")?;

    Ok(row.and_then(|r| r.account_id))
}

/// Returns the `(eve_character_id, name)` of the main character for `account_id`,
/// or `None` if the account has no characters yet. Used by the audit module to
/// snapshot the actor character at write time.
pub async fn get_main_for_account_tx(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
) -> Result<Option<(i64, String)>> {
    let row = sqlx::query!(
        "SELECT eve_character_id, name FROM eve_character
         WHERE account_id = $1 AND is_main = TRUE
         LIMIT 1",
        account_id
    )
    .fetch_optional(&mut **tx)
    .await
    .context("failed to fetch main character for account")?;

    Ok(row.map(|r| (r.eve_character_id, r.name)))
}

/// The stored EVE-token material for a character, needed to perform an
/// authenticated ESI call on its behalf. The access/refresh tokens are returned
/// still-encrypted; the caller decrypts transiently.
pub struct CharacterTokenMaterial {
    pub eve_character_id: i64,
    pub encrypted_access_token: Option<Vec<u8>>,
    pub encrypted_refresh_token: Option<Vec<u8>>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub scopes: Vec<String>,
}

/// Reads the token material for an account's main character, or `None` if the
/// account has no main. Used to perform an authenticated ESI call (e.g. the
/// admin character search) on behalf of the admin's own main.
pub async fn get_main_token_material(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<Option<CharacterTokenMaterial>> {
    let row = sqlx::query!(
        r#"
        SELECT eve_character_id, encrypted_access_token, encrypted_refresh_token,
               access_token_expires_at, scopes
        FROM eve_character
        WHERE account_id = $1 AND is_main = TRUE
        LIMIT 1
        "#,
        account_id
    )
    .fetch_optional(pool)
    .await
    .context("failed to fetch main character token material")?;

    Ok(row.map(|r| CharacterTokenMaterial {
        eve_character_id: r.eve_character_id,
        encrypted_access_token: r.encrypted_access_token,
        encrypted_refresh_token: r.encrypted_refresh_token,
        access_token_expires_at: r.access_token_expires_at,
        scopes: r.scopes,
    }))
}

/// Persists refreshed EVE tokens for a character (by `eve_character_id`),
/// encrypting them with `encryption_key`, recording the observed `owner_hash`,
/// and resetting `token_status = 'valid'`. Used after a successful access-token
/// refresh whose owner hash matched. Returns the number of rows updated (0 if
/// the character vanished).
pub async fn update_tokens_by_eve_id(
    pool: &PgPool,
    eve_character_id: i64,
    access_token_plaintext: &str,
    refresh_token_plaintext: &str,
    access_token_expires_at: DateTime<Utc>,
    owner_hash: &str,
    encryption_key: &[u8],
) -> Result<u64> {
    let encrypted_access = crypto::encrypt_token(access_token_plaintext, encryption_key)
        .context("failed to encrypt refreshed access token")?;
    let encrypted_refresh = crypto::encrypt_token(refresh_token_plaintext, encryption_key)
        .context("failed to encrypt refreshed refresh token")?;

    let result = sqlx::query!(
        r#"
        UPDATE eve_character
        SET encrypted_access_token = $2,
            encrypted_refresh_token = $3,
            access_token_expires_at = $4,
            owner_hash = $5,
            token_status = 'valid',
            updated_at = now()
        WHERE eve_character_id = $1
        "#,
        eve_character_id,
        encrypted_access.as_slice(),
        encrypted_refresh.as_slice(),
        access_token_expires_at,
        owner_hash,
    )
    .execute(pool)
    .await
    .context("failed to update refreshed character tokens")?;

    Ok(result.rows_affected())
}

/// A character the daily sweep should attempt to refresh: not already
/// `token_expired` and holding a refresh token. Carries the stored `owner_hash`
/// so the service can compare it against the refreshed token's claim.
pub struct RefreshableCharacter {
    pub eve_character_id: i64,
    pub account_id: Option<Uuid>,
    pub encrypted_refresh_token: Vec<u8>,
    pub owner_hash: Option<String>,
}

/// Every character eligible for the sweep: `token_status <> 'token_expired'`
/// and with a non-null refresh token.
pub async fn list_refreshable(pool: &PgPool) -> Result<Vec<RefreshableCharacter>> {
    let rows = sqlx::query!(
        r#"
        SELECT eve_character_id, account_id, encrypted_refresh_token, owner_hash
        FROM eve_character
        WHERE token_status <> 'token_expired'
          AND encrypted_refresh_token IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await
    .context("failed to list refreshable characters")?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            r.encrypted_refresh_token.map(|token| RefreshableCharacter {
                eve_character_id: r.eve_character_id,
                account_id: r.account_id,
                encrypted_refresh_token: token,
                owner_hash: r.owner_hash,
            })
        })
        .collect())
}

/// Sets a character's `token_status` and NULLs its credential columns, recording
/// the presented `owner_hash` (so an `owner_mismatch` row stores the new owner's
/// hash). The status string is validated by the column's CHECK constraint.
/// Returns the number of rows updated.
pub async fn set_token_status(
    pool: &PgPool,
    eve_character_id: i64,
    token_status: &str,
    owner_hash: Option<&str>,
) -> Result<u64> {
    let result = sqlx::query!(
        r#"
        UPDATE eve_character
        SET token_status = $2,
            owner_hash = COALESCE($3, owner_hash),
            encrypted_access_token = NULL,
            encrypted_refresh_token = NULL,
            access_token_expires_at = NULL,
            scopes = '{}',
            updated_at = now()
        WHERE eve_character_id = $1
        "#,
        eve_character_id,
        token_status,
        owner_hash,
    )
    .execute(pool)
    .await
    .context("failed to set character token status")?;

    Ok(result.rows_affected())
}

/// Expires every still-`valid` character of an account: sets
/// `token_status = 'token_expired'` and NULLs credentials. Backs the sweep's
/// 7-day idle waterfall. Returns the number of rows affected.
pub async fn expire_valid_tokens_for_account(pool: &PgPool, account_id: Uuid) -> Result<u64> {
    let result = sqlx::query!(
        r#"
        UPDATE eve_character
        SET token_status = 'token_expired',
            encrypted_access_token = NULL,
            encrypted_refresh_token = NULL,
            access_token_expires_at = NULL,
            scopes = '{}',
            updated_at = now()
        WHERE account_id = $1 AND token_status = 'valid'
        "#,
        account_id
    )
    .execute(pool)
    .await
    .context("failed to expire idle account tokens")?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;

    fn test_key() -> Vec<u8> {
        vec![0u8; 32]
    }

    #[sqlx::test]
    async fn create_orphan_inserts_row(pool: PgPool) {
        let id = create_orphan(&pool, 12345, "Test Pilot", 1000001, "Test Corp", None, None)
            .await
            .unwrap();
        assert!(!id.is_nil());
    }

    #[sqlx::test]
    async fn upsert_tokens_inserts_new_character(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let id = upsert_tokens(
            &mut tx,
            account_id,
            99001,
            "Pilot One",
            1000001,
            "Corp One",
            None,
            None,
            "client1",
            "access_tok",
            "refresh_tok",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert!(!id.is_nil());
    }

    #[sqlx::test]
    async fn upsert_tokens_updates_existing(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let id1 = upsert_tokens(
            &mut tx,
            account_id,
            99002,
            "Pilot Two",
            1000001,
            "Corp One",
            None,
            None,
            "client1",
            "access_tok_v1",
            "refresh_tok_v1",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx2 = pool.begin().await.unwrap();
        let id2 = upsert_tokens(
            &mut tx2,
            account_id,
            99002,
            "Pilot Two Updated",
            1000002,
            "Corp Two",
            None,
            None,
            "client1",
            "access_tok_v2",
            "refresh_tok_v2",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx2.commit().await.unwrap();

        assert_eq!(id1, id2);
        let chars = list_for_account(&pool, account_id).await.unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "Pilot Two Updated");
        assert_eq!(chars[0].corporation_name, "Corp Two");
    }

    #[sqlx::test]
    async fn promote_if_no_main_promotes_first_character(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let char_id = upsert_tokens(
            &mut tx,
            account_id,
            99003,
            "Main Pilot",
            1000001,
            "Corp One",
            None,
            None,
            "client1",
            "access",
            "refresh",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        let promoted = promote_if_no_main(&mut tx, account_id, char_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert!(promoted);
        let chars = list_for_account(&pool, account_id).await.unwrap();
        assert!(chars[0].is_main);
    }

    #[sqlx::test]
    async fn promote_if_no_main_noop_when_main_exists(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let char1 = upsert_tokens(
            &mut tx,
            account_id,
            99004,
            "First",
            1000001,
            "Corp One",
            None,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        promote_if_no_main(&mut tx, account_id, char1)
            .await
            .unwrap();
        let char2 = upsert_tokens(
            &mut tx,
            account_id,
            99005,
            "Second",
            1000001,
            "Corp One",
            None,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        let promoted = promote_if_no_main(&mut tx, account_id, char2)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert!(!promoted);
        let chars = list_for_account(&pool, account_id).await.unwrap();
        let main_count = chars.iter().filter(|c| c.is_main).count();
        assert_eq!(main_count, 1);
        assert_eq!(chars.iter().find(|c| c.is_main).unwrap().id, char1);
    }

    #[sqlx::test]
    async fn delete_character_returns_true_when_deleted(pool: PgPool) {
        let id = create_orphan(&pool, 99006, "To Delete", 1000001, "Corp", None, None)
            .await
            .unwrap();
        let deleted = delete_character(&pool, id).await.unwrap();
        assert!(deleted);
    }

    #[sqlx::test]
    async fn delete_character_returns_false_when_not_found(pool: PgPool) {
        let deleted = delete_character(&pool, Uuid::new_v4()).await.unwrap();
        assert!(!deleted);
    }

    #[sqlx::test]
    async fn count_for_account_counts_correctly(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        upsert_tokens(
            &mut tx,
            account_id,
            99010,
            "A",
            1000001,
            "Corp One",
            None,
            None,
            "c",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let count = count_for_account(&pool, account_id).await.unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test]
    async fn is_main_returns_none_for_unknown(pool: PgPool) {
        let result = is_main(&pool, Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn clear_tokens_for_account_nulls_credential_columns_only(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        let with_tokens = upsert_tokens(
            &mut tx,
            account_id,
            99100,
            "Has Tokens",
            1000001,
            "Corp One",
            Some(2000001),
            Some("Alliance One"),
            "client1",
            "access_tok",
            "refresh_tok",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &["esi-skills.read_skills.v1".to_string()],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        promote_if_no_main(&mut tx, account_id, with_tokens)
            .await
            .unwrap();
        let without_tokens = upsert_tokens(
            &mut tx,
            account_id,
            99101,
            "Already Clear",
            1000002,
            "Corp Two",
            None,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        clear_tokens_for_account(&mut tx, account_id).await.unwrap();
        tx.commit().await.unwrap();

        // Credential columns: both rows fully cleared.
        let rows = sqlx::query!(
            r#"
            SELECT id, name, corporation_id, corporation_name,
                   alliance_id, alliance_name, eve_character_id, is_main,
                   encrypted_access_token, encrypted_refresh_token,
                   access_token_expires_at, scopes
            FROM eve_character
            WHERE account_id = $1
            ORDER BY eve_character_id ASC
            "#,
            account_id
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert!(row.encrypted_access_token.is_none());
            assert!(row.encrypted_refresh_token.is_none());
            assert!(row.access_token_expires_at.is_none());
            assert!(row.scopes.is_empty());
        }

        // Identity columns: untouched on the row that had tokens.
        let with_tokens_row = rows.iter().find(|r| r.id == with_tokens).unwrap();
        assert_eq!(with_tokens_row.name, "Has Tokens");
        assert_eq!(with_tokens_row.corporation_id, 1000001);
        assert_eq!(with_tokens_row.corporation_name, "Corp One");
        assert_eq!(with_tokens_row.alliance_id, Some(2000001));
        assert_eq!(
            with_tokens_row.alliance_name.as_deref(),
            Some("Alliance One")
        );
        assert_eq!(with_tokens_row.eve_character_id, 99100);
        assert!(with_tokens_row.is_main);

        let without_tokens_row = rows.iter().find(|r| r.id == without_tokens).unwrap();
        assert_eq!(without_tokens_row.name, "Already Clear");
        assert!(!without_tokens_row.is_main);
    }

    #[sqlx::test]
    async fn clear_tokens_for_account_only_touches_target_account(pool: PgPool) {
        let target_account = accounts::create_account(&pool).await.unwrap();
        let other_account = accounts::create_account(&pool).await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        upsert_tokens(
            &mut tx,
            target_account,
            99200,
            "Target Pilot",
            1000001,
            "Corp One",
            None,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &["scope.target".to_string()],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        let other_char = upsert_tokens(
            &mut tx,
            other_account,
            99201,
            "Other Pilot",
            1000001,
            "Corp One",
            None,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &["scope.other".to_string()],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        clear_tokens_for_account(&mut tx, target_account)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let other_row = sqlx::query!(
            r#"
            SELECT encrypted_access_token, encrypted_refresh_token,
                   access_token_expires_at, scopes
            FROM eve_character WHERE id = $1
            "#,
            other_char
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(other_row.encrypted_access_token.is_some());
        assert!(other_row.encrypted_refresh_token.is_some());
        assert!(other_row.access_token_expires_at.is_some());
        assert_eq!(other_row.scopes, vec!["scope.other".to_string()]);
    }

    #[sqlx::test]
    async fn get_main_for_account_tx_returns_main(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let main_char = upsert_tokens(
            &mut tx,
            account_id,
            42_000,
            "Main Pilot",
            1_000_001,
            "Corp One",
            None,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        promote_if_no_main(&mut tx, account_id, main_char)
            .await
            .unwrap();
        // A second, non-main character should not be returned.
        let _alt = upsert_tokens(
            &mut tx,
            account_id,
            42_001,
            "Alt Pilot",
            1_000_001,
            "Corp One",
            None,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        let main = get_main_for_account_tx(&mut tx, account_id).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(main, Some((42_000_i64, "Main Pilot".to_string())));
    }

    #[sqlx::test]
    async fn get_main_for_account_tx_returns_none_when_no_main(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        let main = get_main_for_account_tx(&mut tx, account_id).await.unwrap();
        tx.commit().await.unwrap();

        assert!(main.is_none());
    }

    #[sqlx::test]
    async fn search_by_name_matches_case_insensitively(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        upsert_tokens(
            &mut tx,
            account_id,
            7001,
            "Captain Pilgrim",
            1,
            "Corp",
            None,
            None,
            "c",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        create_orphan(&pool, 7002, "Other Soul", 1, "Corp", None, None)
            .await
            .unwrap();

        // Case-insensitive substring "pil" matches "Captain Pilgrim" only.
        let results = search_by_name(&pool, "pil", 50).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].eve_character_id, 7001);
        assert_eq!(results[0].account_id, Some(account_id));
    }

    #[sqlx::test]
    async fn search_by_name_caps_at_limit(pool: PgPool) {
        for i in 0..5 {
            create_orphan(
                &pool,
                8000 + i,
                &format!("Pilot {i}"),
                1,
                "Corp",
                None,
                None,
            )
            .await
            .unwrap();
        }
        let results = search_by_name(&pool, "pilot", 3).await.unwrap();
        assert_eq!(results.len(), 3, "result set is capped at the limit");
    }

    #[sqlx::test]
    async fn search_by_name_treats_wildcards_literally(pool: PgPool) {
        // A pilot whose name literally contains a percent sign.
        create_orphan(&pool, 9001, "100% Legit", 1, "Corp", None, None)
            .await
            .unwrap();
        create_orphan(&pool, 9002, "Totally Normal", 1, "Corp", None, None)
            .await
            .unwrap();

        // A bare "%" must NOT match everything (escaped to a literal); it should
        // match only the name actually containing a percent sign, and not error.
        let results = search_by_name(&pool, "%", 50).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].eve_character_id, 9001);

        // Likewise "_" is a literal underscore, matching nothing here.
        let results = search_by_name(&pool, "_", 50).await.unwrap();
        assert!(results.is_empty());
    }

    #[sqlx::test]
    async fn find_account_for_eve_character_owned_orphan_unknown(pool: PgPool) {
        // Owned.
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        upsert_tokens(
            &mut tx,
            account_id,
            6001,
            "Owned",
            1,
            "Corp",
            None,
            None,
            "c",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &[],
            "owner-hash",
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            find_account_for_eve_character(&pool, 6001).await.unwrap(),
            Some(account_id)
        );

        // Orphan row (account_id NULL) → None.
        create_orphan(&pool, 6002, "Orphan", 1, "Corp", None, None)
            .await
            .unwrap();
        assert_eq!(
            find_account_for_eve_character(&pool, 6002).await.unwrap(),
            None
        );

        // Never-seen → None.
        assert_eq!(
            find_account_for_eve_character(&pool, 6003).await.unwrap(),
            None
        );
    }

    /// Inserts a token-bearing character for an account and commits.
    async fn seed_char(pool: &PgPool, account_id: Uuid, eve_id: i64, owner: &str) {
        let mut tx = pool.begin().await.unwrap();
        upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            "Pilot",
            1,
            "Corp",
            None,
            None,
            "c",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &["scope.a".to_string()],
            owner,
            &test_key(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    /// Reads (token_status, owner_hash, has_refresh_token) for a character.
    async fn token_state(pool: &PgPool, eve_id: i64) -> (String, Option<String>, bool) {
        let r = sqlx::query!(
            "SELECT token_status, owner_hash, encrypted_refresh_token
             FROM eve_character WHERE eve_character_id = $1",
            eve_id
        )
        .fetch_one(pool)
        .await
        .unwrap();
        (
            r.token_status,
            r.owner_hash,
            r.encrypted_refresh_token.is_some(),
        )
    }

    #[sqlx::test]
    async fn upsert_records_owner_hash_and_valid_status(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        seed_char(&pool, account_id, 7001, "hash-1").await;
        let (status, owner, has_token) = token_state(&pool, 7001).await;
        assert_eq!(status, "valid");
        assert_eq!(owner.as_deref(), Some("hash-1"));
        assert!(has_token);
    }

    #[sqlx::test]
    async fn set_token_status_flags_and_nulls_credentials(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        seed_char(&pool, account_id, 7002, "hash-old").await;

        let n = set_token_status(&pool, 7002, "owner_mismatch", Some("hash-new"))
            .await
            .unwrap();
        assert_eq!(n, 1);

        let (status, owner, has_token) = token_state(&pool, 7002).await;
        assert_eq!(status, "owner_mismatch");
        // The presented new hash overwrites the stored one.
        assert_eq!(owner.as_deref(), Some("hash-new"));
        // Credentials are wiped.
        assert!(!has_token);
    }

    #[sqlx::test]
    async fn set_token_status_keeps_owner_hash_when_none_given(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        seed_char(&pool, account_id, 7003, "hash-keep").await;

        set_token_status(&pool, 7003, "token_expired", None)
            .await
            .unwrap();
        let (status, owner, _) = token_state(&pool, 7003).await;
        assert_eq!(status, "token_expired");
        assert_eq!(owner.as_deref(), Some("hash-keep"));
    }

    #[sqlx::test]
    async fn list_refreshable_excludes_expired_and_tokenless(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        // valid + token → included
        seed_char(&pool, account_id, 7101, "h-a").await;
        // owner_mismatch is NOT token_expired but set_token_status NULLs the
        // token, so it is excluded for lacking a refresh token.
        seed_char(&pool, account_id, 7102, "h-b").await;
        set_token_status(&pool, 7102, "owner_mismatch", None)
            .await
            .unwrap();
        // token_expired → excluded
        seed_char(&pool, account_id, 7103, "h-c").await;
        set_token_status(&pool, 7103, "token_expired", None)
            .await
            .unwrap();
        // orphan (no token, default valid) → excluded for lacking a token
        create_orphan(&pool, 7104, "Orphan", 1, "Corp", None, None)
            .await
            .unwrap();

        let refreshable = list_refreshable(&pool).await.unwrap();
        let ids: Vec<i64> = refreshable.iter().map(|c| c.eve_character_id).collect();
        assert_eq!(ids, vec![7101]);
        assert_eq!(refreshable[0].owner_hash.as_deref(), Some("h-a"));
    }

    #[sqlx::test]
    async fn expire_valid_tokens_for_account_only_touches_valid(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        seed_char(&pool, account_id, 7201, "h-1").await; // valid
        seed_char(&pool, account_id, 7202, "h-2").await;
        set_token_status(&pool, 7202, "owner_mismatch", None)
            .await
            .unwrap(); // already flagged

        let other = accounts::create_account(&pool).await.unwrap();
        seed_char(&pool, other, 7203, "h-3").await; // different account

        let n = expire_valid_tokens_for_account(&pool, account_id)
            .await
            .unwrap();
        assert_eq!(n, 1); // only the valid one

        assert_eq!(token_state(&pool, 7201).await.0, "token_expired");
        assert_eq!(token_state(&pool, 7202).await.0, "owner_mismatch"); // untouched
        assert_eq!(token_state(&pool, 7203).await.0, "valid"); // other account untouched
    }
}
