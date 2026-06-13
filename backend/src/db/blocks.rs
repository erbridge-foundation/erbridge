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

/// Removes a block row. Returns the deleted row's `character_name` snapshot
/// (the outer `Option` discriminates "no row matched" — mapped to a 404 by the
/// caller — from a matched row whose `character_name` may itself be NULL).
pub async fn delete_block(
    tx: &mut Transaction<'_, Postgres>,
    eve_character_id: i64,
) -> Result<Option<Option<String>>> {
    let row = sqlx::query!(
        "DELETE FROM blocked_eve_character WHERE eve_character_id = $1
         RETURNING character_name",
        eve_character_id
    )
    .fetch_optional(&mut **tx)
    .await
    .context("failed to delete block")?;
    Ok(row.map(|r| r.character_name))
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

/// The subset of `eve_character_ids` that are blocked, as a set, in a single
/// query (`WHERE eve_character_id = ANY($1)`). Backs the admin search's
/// `already_blocked` annotation without one query per result. Membership in the
/// returned set is the block flag; ids not in the set are unblocked.
pub async fn blocked_set(
    pool: &PgPool,
    eve_character_ids: &[i64],
) -> Result<std::collections::HashSet<i64>> {
    let rows = sqlx::query!(
        "SELECT eve_character_id FROM blocked_eve_character WHERE eve_character_id = ANY($1)",
        eve_character_ids
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch blocked set")?;

    Ok(rows.into_iter().map(|r| r.eve_character_id).collect())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;

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
        // Matched row → Some; its snapshotted character_name is returned.
        assert_eq!(deleted, Some(Some("Griefer".to_string())));
        assert!(list_blocks(&pool).await.unwrap().is_empty());
    }

    #[sqlx::test]
    async fn delete_missing_reports_miss(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let deleted = delete_block(&mut tx, 999).await.unwrap();
        tx.commit().await.unwrap();
        assert!(deleted.is_none());
    }

    #[sqlx::test]
    async fn is_eve_character_blocked_true_and_false(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        block(&pool, 42, admin).await;
        assert!(is_eve_character_blocked(&pool, 42).await.unwrap());
        assert!(!is_eve_character_blocked(&pool, 43).await.unwrap());
    }

    #[sqlx::test]
    async fn blocked_set_returns_only_blocked_ids(pool: PgPool) {
        let admin = accounts::create_account(&pool).await.unwrap();
        block(&pool, 100, admin).await;
        block(&pool, 300, admin).await;

        // Query a mixed set: 100 and 300 are blocked; 200 and 400 are not.
        let set = blocked_set(&pool, &[100, 200, 300, 400]).await.unwrap();
        assert!(set.contains(&100));
        assert!(set.contains(&300));
        assert!(!set.contains(&200));
        assert!(!set.contains(&400));
        assert_eq!(set.len(), 2);
    }

    #[sqlx::test]
    async fn blocked_set_empty_input_is_empty(pool: PgPool) {
        let set = blocked_set(&pool, &[]).await.unwrap();
        assert!(set.is_empty());
    }
}
