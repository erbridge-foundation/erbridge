use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// A row from `blocked_eve_character` — the self-contained snapshot of a blocked
/// pilot. Never joins to `eve_character` (the snapshot is the source of truth
/// for display).
pub struct BlockedEveCharacter {
    pub eve_character_id: i64,
    pub character_name: Option<String>,
    pub corporation_name: Option<String>,
    pub reason: Option<String>,
    pub blocked_by: Option<Uuid>,
    pub blocked_at: DateTime<Utc>,
}

/// Inserts a block row. Idempotent: a conflict on the existing
/// `eve_character_id` primary key is a no-op and returns `false` (so the caller
/// knows not to emit an audit event); a fresh insert returns `true`.
#[allow(clippy::too_many_arguments)]
pub async fn insert_block(
    tx: &mut Transaction<'_, Postgres>,
    eve_character_id: i64,
    character_name: Option<&str>,
    corporation_name: Option<&str>,
    reason: Option<&str>,
    blocked_by: Uuid,
) -> Result<bool> {
    let result = sqlx::query!(
        r#"
        INSERT INTO blocked_eve_character
            (eve_character_id, character_name, corporation_name, reason, blocked_by)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (eve_character_id) DO NOTHING
        "#,
        eve_character_id,
        character_name,
        corporation_name,
        reason,
        blocked_by,
    )
    .execute(&mut **tx)
    .await
    .context("failed to insert block")?;
    Ok(result.rows_affected() > 0)
}

/// Removes a block row. Returns `true` if a row was deleted, `false` if no row
/// matched (so the caller maps that to a 404).
pub async fn delete_block(
    tx: &mut Transaction<'_, Postgres>,
    eve_character_id: i64,
) -> Result<bool> {
    let result = sqlx::query!(
        "DELETE FROM blocked_eve_character WHERE eve_character_id = $1",
        eve_character_id
    )
    .execute(&mut **tx)
    .await
    .context("failed to delete block")?;
    Ok(result.rows_affected() > 0)
}

/// All block rows, newest first. Reads flat — no join to `eve_character`.
pub async fn list_blocks(pool: &PgPool) -> Result<Vec<BlockedEveCharacter>> {
    let rows = sqlx::query!(
        r#"
        SELECT eve_character_id, character_name, corporation_name, reason,
               blocked_by, blocked_at
        FROM blocked_eve_character
        ORDER BY blocked_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .context("failed to list blocks")?;

    Ok(rows
        .into_iter()
        .map(|r| BlockedEveCharacter {
            eve_character_id: r.eve_character_id,
            character_name: r.character_name,
            corporation_name: r.corporation_name,
            reason: r.reason,
            blocked_by: r.blocked_by,
            blocked_at: r.blocked_at,
        })
        .collect())
}

/// Whether a specific EVE character id is in the block list. Used by the SSO
/// callback before any account/character write.
pub async fn is_eve_character_blocked(pool: &PgPool, eve_character_id: i64) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT EXISTS (
            SELECT 1 FROM blocked_eve_character WHERE eve_character_id = $1
        ) AS "exists!""#,
        eve_character_id
    )
    .fetch_one(pool)
    .await
    .context("failed to check character block status")?;
    Ok(row.exists)
}

/// Whether an account owns at least one blocked character — the derived
/// "account is blocked" rule. Used by the bearer branch of `AuthenticatedAccount`.
pub async fn account_has_blocked_character(pool: &PgPool, account_id: Uuid) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT EXISTS (
            SELECT 1
            FROM eve_character c
            JOIN blocked_eve_character b ON b.eve_character_id = c.eve_character_id
            WHERE c.account_id = $1
        ) AS "exists!""#,
        account_id
    )
    .fetch_one(pool)
    .await
    .context("failed to check account block status")?;
    Ok(row.exists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;

    /// Inserts a bare `eve_character` row bound to `account_id` so the join-based
    /// queries have something to find. Mirrors the minimal columns other db
    /// tests insert directly.
    async fn bind_character(pool: &PgPool, account_id: Uuid, eve_id: i64, name: &str) {
        sqlx::query!(
            "INSERT INTO eve_character (account_id, eve_character_id, name, corporation_id, corporation_name)
             VALUES ($1, $2, $3, 1_000_001, 'Test Corp')",
            account_id,
            eve_id,
            name,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn block(pool: &PgPool, eve_id: i64, by: Uuid) -> bool {
        let mut tx = pool.begin().await.unwrap();
        let inserted = insert_block(
            &mut tx,
            eve_id,
            Some("Griefer"),
            Some("Bad Corp"),
            Some("botting"),
            by,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        inserted
    }

    #[sqlx::test]
    async fn insert_then_list_returns_row(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        assert!(block(&pool, 42, admin).await);

        let blocks = list_blocks(&pool).await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].eve_character_id, 42);
        assert_eq!(blocks[0].character_name.as_deref(), Some("Griefer"));
        assert_eq!(blocks[0].corporation_name.as_deref(), Some("Bad Corp"));
        assert_eq!(blocks[0].reason.as_deref(), Some("botting"));
        assert_eq!(blocks[0].blocked_by, Some(admin));
    }

    #[sqlx::test]
    async fn insert_is_idempotent(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        assert!(block(&pool, 42, admin).await, "first insert is new");
        assert!(!block(&pool, 42, admin).await, "second insert is a no-op");
        assert_eq!(list_blocks(&pool).await.unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn insert_with_null_snapshot_fields_succeeds(pool: PgPool) {
        // ESI-unavailable case: name/corp left NULL, block still effective.
        let admin = accounts::create_account(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let inserted = insert_block(&mut tx, 7, None, None, None, admin)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert!(inserted);

        let blocks = list_blocks(&pool).await.unwrap();
        assert!(blocks[0].character_name.is_none());
        assert!(blocks[0].corporation_name.is_none());
        assert!(is_eve_character_blocked(&pool, 7).await.unwrap());
    }

    #[sqlx::test]
    async fn list_is_newest_first(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        block(&pool, 1, admin).await;
        block(&pool, 2, admin).await;
        let blocks = list_blocks(&pool).await.unwrap();
        // blocked_at DESC — the most recently inserted (2) comes first.
        assert_eq!(blocks[0].eve_character_id, 2);
        assert_eq!(blocks[1].eve_character_id, 1);
    }

    #[sqlx::test]
    async fn delete_removes_row_and_reports_hit(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        block(&pool, 42, admin).await;

        let mut tx = pool.begin().await.unwrap();
        let deleted = delete_block(&mut tx, 42).await.unwrap();
        tx.commit().await.unwrap();
        assert!(deleted);
        assert!(list_blocks(&pool).await.unwrap().is_empty());
    }

    #[sqlx::test]
    async fn delete_missing_reports_miss(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let deleted = delete_block(&mut tx, 999).await.unwrap();
        tx.commit().await.unwrap();
        assert!(!deleted);
    }

    #[sqlx::test]
    async fn is_eve_character_blocked_true_and_false(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        block(&pool, 42, admin).await;
        assert!(is_eve_character_blocked(&pool, 42).await.unwrap());
        assert!(!is_eve_character_blocked(&pool, 43).await.unwrap());
    }

    #[sqlx::test]
    async fn account_has_blocked_character_true_when_owned_char_blocked(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        let victim = accounts::create_account(&pool).await.unwrap();
        bind_character(&pool, victim, 500, "Alt").await;
        block(&pool, 500, admin).await;

        assert!(account_has_blocked_character(&pool, victim).await.unwrap());
    }

    #[sqlx::test]
    async fn account_has_blocked_character_false_when_no_owned_block(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        let other = accounts::create_account(&pool).await.unwrap();
        bind_character(&pool, other, 500, "Clean Pilot").await;
        // Block a *different*, unowned character id.
        block(&pool, 999, admin).await;

        assert!(!account_has_blocked_character(&pool, other).await.unwrap());
    }

    #[sqlx::test]
    async fn account_has_blocked_character_false_for_block_of_unowned_id(pool: PgPool) {
        // A block exists, but the character row binding it to an account does
        // not (orphan/never-seen) — so no account "has" it.
        let admin = accounts::create_account(&pool).await.unwrap();
        block(&pool, 12345, admin).await;
        assert!(!account_has_blocked_character(&pool, admin).await.unwrap());
    }
}
