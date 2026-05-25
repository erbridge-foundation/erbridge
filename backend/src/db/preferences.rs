use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Fetch an account's preference bag. Returns `None` if the account does not
/// exist. A freshly-created account has `{}`.
pub async fn get_preferences(pool: &PgPool, account_id: Uuid) -> Result<Option<Value>> {
    let row = sqlx::query!("SELECT preferences FROM account WHERE id = $1", account_id)
        .fetch_optional(pool)
        .await
        .context("failed to fetch account preferences")?;

    Ok(row.map(|r| r.preferences))
}

/// Shallow-merge `patch` into the account's existing preference bag (top-level
/// keys in `patch` overwrite existing keys; keys absent from `patch` are left
/// untouched) and return the merged bag. Uses Postgres' `||` jsonb concatenation
/// so the merge is atomic in a single round-trip. Returns `None` if the account
/// does not exist.
///
/// No validation happens here — the service layer is responsible for rejecting
/// unknown keys and invalid values before calling this.
pub async fn merge_preferences(
    pool: &PgPool,
    account_id: Uuid,
    patch: &Value,
) -> Result<Option<Value>> {
    let row = sqlx::query!(
        "UPDATE account
         SET preferences = preferences || $2, updated_at = now()
         WHERE id = $1
         RETURNING preferences",
        account_id,
        patch,
    )
    .fetch_optional(pool)
    .await
    .context("failed to merge account preferences")?;

    Ok(row.map(|r| r.preferences))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;
    use serde_json::json;

    #[sqlx::test]
    async fn get_preferences_defaults_to_empty_object(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        let prefs = get_preferences(&pool, id).await.unwrap().unwrap();
        assert_eq!(prefs, json!({}));
    }

    #[sqlx::test]
    async fn get_preferences_returns_none_for_missing_account(pool: PgPool) {
        let prefs = get_preferences(&pool, Uuid::new_v4()).await.unwrap();
        assert!(prefs.is_none());
    }

    #[sqlx::test]
    async fn merge_preferences_sets_keys(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        let merged = merge_preferences(&pool, id, &json!({"text_size": "large"}))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(merged, json!({"text_size": "large"}));
    }

    #[sqlx::test]
    async fn merge_preferences_preserves_other_keys(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        merge_preferences(&pool, id, &json!({"text_size": "large"}))
            .await
            .unwrap();
        let merged = merge_preferences(&pool, id, &json!({"reduce_motion": "on"}))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(merged, json!({"text_size": "large", "reduce_motion": "on"}));
    }

    #[sqlx::test]
    async fn merge_preferences_overwrites_existing_key(pool: PgPool) {
        let id = accounts::create_account(&pool).await.unwrap();
        merge_preferences(&pool, id, &json!({"text_size": "large"}))
            .await
            .unwrap();
        let merged = merge_preferences(&pool, id, &json!({"text_size": "small"}))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(merged, json!({"text_size": "small"}));
    }

    #[sqlx::test]
    async fn merge_preferences_returns_none_for_missing_account(pool: PgPool) {
        let merged = merge_preferences(&pool, Uuid::new_v4(), &json!({"text_size": "large"}))
            .await
            .unwrap();
        assert!(merged.is_none());
    }
}
