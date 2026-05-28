use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::DbError;

#[allow(dead_code)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub scope: String,
    pub name: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

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
pub async fn insert_key(
    pool: &PgPool,
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
    .fetch_one(pool)
    .await?;

    Ok((row.id, row.created_at))
}

/// Looks up a key row by its pre-computed SHA-256 hex hash.
/// Returns `None` if not found or expired.
pub async fn find_by_hash(pool: &PgPool, key_hash: &str) -> Result<Option<ApiKeyRow>> {
    let row = sqlx::query!(
        r#"
        SELECT id, account_id, scope, name, expires_at, created_at
        FROM api_key
        WHERE key_hash = $1
          AND (expires_at IS NULL OR expires_at > now())
        "#,
        key_hash,
    )
    .fetch_optional(pool)
    .await
    .context("failed to look up api key by hash")?;

    Ok(row.map(|r| ApiKeyRow {
        id: r.id,
        account_id: r.account_id,
        scope: r.scope,
        name: r.name,
        expires_at: r.expires_at,
        created_at: r.created_at,
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

/// Deletes a key by id, only if it belongs to the given account with `scope = 'account'`.
/// Returns `true` if a row was deleted.
pub async fn delete_for_account(pool: &PgPool, id: Uuid, account_id: Uuid) -> Result<bool> {
    let result = sqlx::query!(
        r#"
        DELETE FROM api_key
        WHERE id = $1
          AND account_id = $2
          AND scope = 'account'
        "#,
        id,
        account_id,
    )
    .execute(pool)
    .await
    .context("failed to delete api key")?;

    Ok(result.rows_affected() > 0)
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
        assert!(deleted);
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
        assert!(!deleted);
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
