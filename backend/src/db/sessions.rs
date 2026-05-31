use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub struct SessionRow {
    pub session_id: String,
    pub account_id: Uuid,
    pub csrf_state: Option<String>,
    pub add_character_mode: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn insert(
    pool: &PgPool,
    session_id: &str,
    account_id: Uuid,
    csrf_state: Option<&str>,
    add_character_mode: bool,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO session (session_id, account_id, csrf_state, add_character_mode, expires_at)
         VALUES ($1, $2, $3, $4, now() + interval '7 days')",
        session_id,
        account_id,
        csrf_state,
        add_character_mode,
    )
    .execute(pool)
    .await
    .context("failed to insert session")?;
    Ok(())
}

/// Atomically refreshes `last_seen_at` and `expires_at` on a non-expired row and
/// returns it. Returns `None` if no row matched (missing or already expired).
pub async fn refresh_and_get(pool: &PgPool, session_id: &str) -> Result<Option<SessionRow>> {
    let row = sqlx::query!(
        "UPDATE session
         SET last_seen_at = now(),
             expires_at = now() + interval '7 days'
         WHERE session_id = $1 AND expires_at > now()
         RETURNING session_id, account_id, csrf_state, add_character_mode,
                   created_at, last_seen_at, expires_at",
        session_id,
    )
    .fetch_optional(pool)
    .await
    .context("failed to refresh session")?;

    Ok(row.map(|r| SessionRow {
        session_id: r.session_id,
        account_id: r.account_id,
        csrf_state: r.csrf_state,
        add_character_mode: r.add_character_mode,
        created_at: r.created_at,
        last_seen_at: r.last_seen_at,
        expires_at: r.expires_at,
    }))
}

pub async fn delete(pool: &PgPool, session_id: &str) -> Result<()> {
    sqlx::query!("DELETE FROM session WHERE session_id = $1", session_id)
        .execute(pool)
        .await
        .context("failed to delete session")?;
    Ok(())
}

pub async fn delete_for_account(pool: &PgPool, account_id: Uuid) -> Result<u64> {
    let result = sqlx::query!("DELETE FROM session WHERE account_id = $1", account_id)
        .execute(pool)
        .await
        .context("failed to delete sessions for account")?;
    Ok(result.rows_affected())
}

/// Transactional variant of [`delete_for_account`], so a caller can tear down an
/// account's sessions atomically alongside other writes (e.g. the block
/// transaction, which clears tokens and inserts the block row in one unit).
pub async fn delete_for_account_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
) -> Result<u64> {
    let result = sqlx::query!("DELETE FROM session WHERE account_id = $1", account_id)
        .execute(&mut **tx)
        .await
        .context("failed to delete sessions for account (tx)")?;
    Ok(result.rows_affected())
}

pub async fn list_ids_for_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<String>> {
    let rows = sqlx::query!(
        "SELECT session_id FROM session WHERE account_id = $1 AND expires_at > now()",
        account_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to list session ids")?;

    Ok(rows.into_iter().map(|r| r.session_id).collect())
}

pub async fn delete_expired(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query!("DELETE FROM session WHERE expires_at < now()")
        .execute(pool)
        .await
        .context("failed to delete expired sessions")?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts::create_account;

    async fn make_account(pool: &PgPool) -> Uuid {
        create_account(pool).await.unwrap()
    }

    #[sqlx::test]
    async fn insert_then_refresh_returns_row(pool: PgPool) {
        let account_id = make_account(&pool).await;
        insert(&pool, "sess1", account_id, Some("csrf"), false)
            .await
            .unwrap();

        let row = refresh_and_get(&pool, "sess1").await.unwrap().unwrap();
        assert_eq!(row.session_id, "sess1");
        assert_eq!(row.account_id, account_id);
        assert_eq!(row.csrf_state.as_deref(), Some("csrf"));
        assert!(!row.add_character_mode);
    }

    #[sqlx::test]
    async fn refresh_advances_last_seen_and_expires_at(pool: PgPool) {
        let account_id = make_account(&pool).await;
        insert(&pool, "sess1", account_id, None, false)
            .await
            .unwrap();

        // Manually rewind both timestamps so the refresh has somewhere to advance to.
        sqlx::query!(
            "UPDATE session
             SET last_seen_at = now() - interval '1 hour',
                 expires_at   = now() + interval '6 days'
             WHERE session_id = $1",
            "sess1",
        )
        .execute(&pool)
        .await
        .unwrap();

        let before = sqlx::query!(
            "SELECT last_seen_at, expires_at FROM session WHERE session_id = $1",
            "sess1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let refreshed = refresh_and_get(&pool, "sess1").await.unwrap().unwrap();

        assert!(refreshed.last_seen_at > before.last_seen_at);
        assert!(refreshed.expires_at > before.expires_at);
    }

    #[sqlx::test]
    async fn refresh_of_expired_row_returns_none_and_leaves_row_untouched(pool: PgPool) {
        let account_id = make_account(&pool).await;
        insert(&pool, "sess1", account_id, None, false)
            .await
            .unwrap();

        sqlx::query!(
            "UPDATE session SET expires_at = now() - interval '1 second' WHERE session_id = $1",
            "sess1",
        )
        .execute(&pool)
        .await
        .unwrap();

        let before = sqlx::query!(
            "SELECT last_seen_at, expires_at FROM session WHERE session_id = $1",
            "sess1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let result = refresh_and_get(&pool, "sess1").await.unwrap();
        assert!(result.is_none());

        let after = sqlx::query!(
            "SELECT last_seen_at, expires_at FROM session WHERE session_id = $1",
            "sess1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(before.last_seen_at, after.last_seen_at);
        assert_eq!(before.expires_at, after.expires_at);
    }

    #[sqlx::test]
    async fn refresh_missing_returns_none(pool: PgPool) {
        let result = refresh_and_get(&pool, "nope").await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn delete_removes_row(pool: PgPool) {
        let account_id = make_account(&pool).await;
        insert(&pool, "sess1", account_id, None, false)
            .await
            .unwrap();
        delete(&pool, "sess1").await.unwrap();
        assert!(refresh_and_get(&pool, "sess1").await.unwrap().is_none());
    }

    #[sqlx::test]
    async fn list_ids_for_account_excludes_expired(pool: PgPool) {
        let account_id = make_account(&pool).await;
        insert(&pool, "active", account_id, None, false)
            .await
            .unwrap();
        insert(&pool, "expired", account_id, None, false)
            .await
            .unwrap();
        sqlx::query!(
            "UPDATE session SET expires_at = now() - interval '1 second' WHERE session_id = $1",
            "expired",
        )
        .execute(&pool)
        .await
        .unwrap();

        let ids = list_ids_for_account(&pool, account_id).await.unwrap();
        assert_eq!(ids, vec!["active".to_string()]);
    }

    #[sqlx::test]
    async fn delete_for_account_removes_all_rows_for_account(pool: PgPool) {
        let a = make_account(&pool).await;
        let b = make_account(&pool).await;
        insert(&pool, "a1", a, None, false).await.unwrap();
        insert(&pool, "a2", a, None, false).await.unwrap();
        insert(&pool, "b1", b, None, false).await.unwrap();

        let removed = delete_for_account(&pool, a).await.unwrap();
        assert_eq!(removed, 2);

        assert!(list_ids_for_account(&pool, a).await.unwrap().is_empty());
        assert_eq!(
            list_ids_for_account(&pool, b).await.unwrap(),
            vec!["b1".to_string()]
        );
    }

    #[sqlx::test]
    async fn delete_for_account_in_tx_removes_rows_and_rolls_back(pool: PgPool) {
        let a = make_account(&pool).await;
        insert(&pool, "a1", a, None, false).await.unwrap();
        insert(&pool, "a2", a, None, false).await.unwrap();

        // Rollback leaves the rows intact (proves it participates in the tx).
        let mut tx = pool.begin().await.unwrap();
        let removed = delete_for_account_in_tx(&mut tx, a).await.unwrap();
        assert_eq!(removed, 2);
        tx.rollback().await.unwrap();
        assert_eq!(list_ids_for_account(&pool, a).await.unwrap().len(), 2);

        // Commit removes them.
        let mut tx = pool.begin().await.unwrap();
        delete_for_account_in_tx(&mut tx, a).await.unwrap();
        tx.commit().await.unwrap();
        assert!(list_ids_for_account(&pool, a).await.unwrap().is_empty());
    }

    #[sqlx::test]
    async fn delete_expired_removes_only_expired(pool: PgPool) {
        let account_id = make_account(&pool).await;
        insert(&pool, "active", account_id, None, false)
            .await
            .unwrap();
        insert(&pool, "expired", account_id, None, false)
            .await
            .unwrap();
        sqlx::query!(
            "UPDATE session SET expires_at = now() - interval '1 second' WHERE session_id = $1",
            "expired",
        )
        .execute(&pool)
        .await
        .unwrap();

        let removed = delete_expired(&pool).await.unwrap();
        assert_eq!(removed, 1);

        let ids = list_ids_for_account(&pool, account_id).await.unwrap();
        assert_eq!(ids, vec!["active".to_string()]);
    }
}
