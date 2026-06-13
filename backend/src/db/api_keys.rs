use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::db::DbError;

pub struct ApiKeyMetadata {
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Inserts an account-scoped API key row. The caller is responsible for generating the hash.
/// Returns `(id, created_at)`. A unique constraint failure surfaces as
/// `DbError::UniqueViolation` so callers can distinguish it from other errors.
///
/// Generic over the executor so a pool (tests) and a transaction (the service,
/// committing the insert alongside an audit emission) share one query — see the
/// no-new-twins rule in `db/mod.rs`.
pub async fn insert_key(
    executor: impl PgExecutor<'_>,
    account_id: Uuid,
    name: &str,
    key_hash: &str,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(Uuid, DateTime<Utc>), DbError> {
    let row = sqlx::query!(
        r#"
        INSERT INTO api_key (account_id, scope, name, key_hash, expires_at)
        VALUES ($1, 'account', $2, $3, $4)
        RETURNING id, created_at
        "#,
        account_id,
        name,
        key_hash,
        expires_at,
    )
    .fetch_one(executor)
    .await?;

    Ok((row.id, row.created_at))
}

/// A key row joined with everything the bearer-auth path needs to make its
/// authorisation decision in a single round-trip: the key's `scope`/`account_id`
/// plus the owning account's `status` and whether that account owns a blocked
/// character. The account fields are `Option` because `account_id` may be NULL
/// (server-scoped keys), in which case the LEFT JOIN yields no account row.
pub struct BearerKeyRow {
    pub scope: String,
    pub account_id: Option<Uuid>,
    /// `account.status`, or `None` if the key has no owning account row.
    pub account_status: Option<String>,
    /// Whether the owning account owns at least one blocked character.
    pub account_blocked: bool,
}

/// Looks up a key row by its pre-computed SHA-256 hex hash, joining the owning
/// account's status and block state so the bearer extractor authorises in one
/// query. Returns `None` if no live (unexpired) key matches.
pub async fn find_by_hash(pool: &PgPool, key_hash: &str) -> Result<Option<BearerKeyRow>> {
    let row = sqlx::query!(
        r#"
        SELECT
            k.scope,
            k.account_id,
            a.status AS "account_status?",
            EXISTS (
                SELECT 1
                FROM eve_character c
                JOIN blocked_eve_character b ON b.eve_character_id = c.eve_character_id
                WHERE c.account_id = k.account_id
            ) AS "account_blocked!"
        FROM api_key k
        LEFT JOIN account a ON a.id = k.account_id
        WHERE k.key_hash = $1
          AND (k.expires_at IS NULL OR k.expires_at > now())
        "#,
        key_hash,
    )
    .fetch_optional(pool)
    .await
    .context("failed to look up api key by hash")?;

    Ok(row.map(|r| BearerKeyRow {
        scope: r.scope,
        account_id: r.account_id,
        account_status: r.account_status,
        account_blocked: r.account_blocked,
    }))
}

/// Returns metadata for all keys belonging to the given account, ordered by creation time.
/// Does not include `key_hash` or any plaintext.
pub async fn list_for_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<ApiKeyMetadata>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, scope, expires_at, created_at
        FROM api_key
        WHERE account_id = $1
        ORDER BY created_at ASC
        "#,
        account_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to list api keys")?;

    Ok(rows
        .into_iter()
        .map(|r| ApiKeyMetadata {
            id: r.id,
            name: r.name,
            scope: r.scope,
            expires_at: r.expires_at,
            created_at: r.created_at,
        })
        .collect())
}

/// Deletes a key by id, only if it belongs to the given account with
/// `scope = 'account'`. Returns the deleted key's `name` (so the caller can
/// snapshot the label into an audit event), or `None` if no matching key was
/// deleted. Generic over the executor so the service can commit the delete
/// alongside its audit emission in one transaction.
pub async fn delete_for_account(
    executor: impl PgExecutor<'_>,
    id: Uuid,
    account_id: Uuid,
) -> Result<Option<String>> {
    let row = sqlx::query!(
        r#"
        DELETE FROM api_key
        WHERE id = $1
          AND account_id = $2
          AND scope = 'account'
        RETURNING name
        "#,
        id,
        account_id,
    )
    .fetch_optional(executor)
    .await
    .context("failed to delete api key")?;

    Ok(row.map(|r| r.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;
    use sqlx::PgPool;

    fn fake_hash(tag: &str) -> String {
        // Deterministic fake hash for tests that don't exercise the real hasher.
        format!("{:0<64}", tag)
    }

    #[sqlx::test]
    async fn insert_and_list_key(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let (id, created_at) = insert_key(&pool, account_id, "ci", &fake_hash("ci"), None)
            .await
            .unwrap();

        assert!(!id.is_nil());
        assert!(created_at <= Utc::now());

        let keys = list_for_account(&pool, account_id).await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].id, id);
        assert_eq!(keys[0].name, "ci");
        assert_eq!(keys[0].scope, "account");
        assert!(keys[0].expires_at.is_none());
    }

    #[sqlx::test]
    async fn find_by_hash_returns_row(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let hash = fake_hash("find_me");
        insert_key(&pool, account_id, "test", &hash, None)
            .await
            .unwrap();

        let row = find_by_hash(&pool, &hash).await.unwrap().unwrap();
        assert_eq!(row.account_id, Some(account_id));
        assert_eq!(row.scope, "account");
        assert_eq!(row.account_status.as_deref(), Some("active"));
        assert!(!row.account_blocked);
    }

    #[sqlx::test]
    async fn find_by_hash_reports_soft_deleted_status(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        accounts::soft_delete(&mut tx, account_id).await.unwrap();
        tx.commit().await.unwrap();
        let hash = fake_hash("soft");
        insert_key(&pool, account_id, "k", &hash, None)
            .await
            .unwrap();

        let row = find_by_hash(&pool, &hash).await.unwrap().unwrap();
        assert_eq!(row.account_status.as_deref(), Some("soft_deleted"));
        assert!(!row.account_blocked);
    }

    #[sqlx::test]
    async fn find_by_hash_reports_blocked_account(pool: PgPool) {
        use crate::db::blocks;
        let admin = accounts::create_account(&pool).await.unwrap();
        let account_id = accounts::create_account(&pool).await.unwrap();
        // Bind a character to the account, then block that character.
        sqlx::query!(
            "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name)
             VALUES ($1, 700, 'Owned', 1_000_001, 'Test Corp')",
            account_id,
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        blocks::insert_block(&mut tx, 700, Some("Owned"), None, None, admin)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let hash = fake_hash("blocked");
        insert_key(&pool, account_id, "k", &hash, None)
            .await
            .unwrap();

        let row = find_by_hash(&pool, &hash).await.unwrap().unwrap();
        assert_eq!(row.account_status.as_deref(), Some("active"));
        assert!(row.account_blocked);
    }

    #[sqlx::test]
    async fn find_by_hash_unknown_returns_none(pool: PgPool) {
        let result = find_by_hash(&pool, &fake_hash("unknown")).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn find_by_hash_expired_returns_none(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let hash = fake_hash("expired");
        insert_key(
            &pool,
            account_id,
            "expired",
            &hash,
            Some(Utc::now() - chrono::Duration::hours(1)),
        )
        .await
        .unwrap();

        let result = find_by_hash(&pool, &hash).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn delete_removes_own_key(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        let hash = fake_hash("delete_me");
        let (id, _) = insert_key(&pool, account_id, "to-delete", &hash, None)
            .await
            .unwrap();

        let deleted = delete_for_account(&pool, id, account_id).await.unwrap();
        assert_eq!(deleted.as_deref(), Some("to-delete"));
        assert!(find_by_hash(&pool, &hash).await.unwrap().is_none());
    }

    #[sqlx::test]
    async fn delete_wrong_account_does_not_remove(pool: PgPool) {
        let account_a = accounts::create_account(&pool).await.unwrap();
        let account_b = accounts::create_account(&pool).await.unwrap();
        let hash = fake_hash("not_yours");
        let (id, _) = insert_key(&pool, account_a, "key", &hash, None)
            .await
            .unwrap();

        let deleted = delete_for_account(&pool, id, account_b).await.unwrap();
        assert!(deleted.is_none());
        assert!(find_by_hash(&pool, &hash).await.unwrap().is_some());
    }

    #[sqlx::test]
    async fn insert_duplicate_name_same_account_returns_unique_violation(pool: PgPool) {
        let account_id = accounts::create_account(&pool).await.unwrap();
        insert_key(&pool, account_id, "ci", &fake_hash("first"), None)
            .await
            .unwrap();

        let err = insert_key(&pool, account_id, "ci", &fake_hash("second"), None)
            .await
            .unwrap_err();

        assert!(matches!(err, DbError::UniqueViolation { .. }));
    }

    #[sqlx::test]
    async fn insert_same_name_different_accounts_succeeds(pool: PgPool) {
        let account_a = accounts::create_account(&pool).await.unwrap();
        let account_b = accounts::create_account(&pool).await.unwrap();

        insert_key(&pool, account_a, "ci", &fake_hash("a"), None)
            .await
            .unwrap();
        insert_key(&pool, account_b, "ci", &fake_hash("b"), None)
            .await
            .unwrap();

        assert_eq!(list_for_account(&pool, account_a).await.unwrap().len(), 1);
        assert_eq!(list_for_account(&pool, account_b).await.unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn list_returns_only_own_keys(pool: PgPool) {
        let account_a = accounts::create_account(&pool).await.unwrap();
        let account_b = accounts::create_account(&pool).await.unwrap();
        insert_key(&pool, account_a, "a-key", &fake_hash("a"), None)
            .await
            .unwrap();
        insert_key(&pool, account_b, "b-key", &fake_hash("b"), None)
            .await
            .unwrap();

        let keys_a = list_for_account(&pool, account_a).await.unwrap();
        assert_eq!(keys_a.len(), 1);
        assert_eq!(keys_a[0].name, "a-key");

        let keys_b = list_for_account(&pool, account_b).await.unwrap();
        assert_eq!(keys_b.len(), 1);
        assert_eq!(keys_b[0].name, "b-key");
    }
}
