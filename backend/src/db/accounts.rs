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

pub async fn reactivate_if_soft_deleted(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<()> {
    sqlx::query!(
        "UPDATE account SET status = 'active', delete_requested_at = NULL
         WHERE id = $1 AND status = 'soft_deleted'",
        id
    )
    .execute(&mut **tx)
    .await
    .context("failed to reactivate account")?;
    Ok(())
}

pub async fn soft_delete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query!(
        "UPDATE account SET status = 'soft_deleted', delete_requested_at = now()
         WHERE id = $1",
        id
    )
    .execute(pool)
    .await
    .context("failed to soft delete account")?;
    Ok(())
}

/// Returns the account that already owns this `eve_character_id` if present, the
/// session's `add_character_account_id` when in add-character mode, or creates a
/// new account row otherwise.
pub async fn resolve_or_create(
    tx: &mut Transaction<'_, Postgres>,
    add_character_account_id: Option<Uuid>,
    eve_character_id: i64,
) -> Result<Uuid> {
    if let Some(account_id) = add_character_account_id {
        return Ok(account_id);
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
            return Ok(account_id);
        }
    }

    // No account found — create a new one.
    let row = sqlx::query!("INSERT INTO account DEFAULT VALUES RETURNING id")
        .fetch_one(&mut **tx)
        .await
        .context("failed to create account")?;
    Ok(row.id)
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
    async fn soft_delete_sets_status(pool: PgPool) {
        let id = create_account(&pool).await.unwrap();
        soft_delete(&pool, id).await.unwrap();
        let account = get_account(&pool, id).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert!(account.delete_requested_at.is_some());
    }
}
