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
    pub alliance_id: Option<i64>,
    pub is_main: bool,
    pub is_online: Option<bool>,
    pub esi_client_id: Option<String>,
    pub esi_token_expires_at: Option<DateTime<Utc>>,
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
    alliance_id: Option<i64>,
    esi_client_id: &str,
    access_token_plaintext: &str,
    refresh_token_plaintext: &str,
    expires_at: DateTime<Utc>,
    encryption_key: &[u8],
) -> Result<Uuid> {
    let encrypted_access = crypto::encrypt_token(access_token_plaintext, encryption_key)
        .context("failed to encrypt access token")?;
    let encrypted_refresh = crypto::encrypt_token(refresh_token_plaintext, encryption_key)
        .context("failed to encrypt refresh token")?;

    let row = sqlx::query!(
        r#"
        INSERT INTO eve_character (
            account_id, eve_character_id, name, corporation_id, alliance_id,
            esi_client_id, encrypted_access_token, encrypted_refresh_token, esi_token_expires_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (eve_character_id) DO UPDATE SET
            account_id = CASE
                WHEN eve_character.account_id IS NULL THEN excluded.account_id
                WHEN eve_character.account_id = excluded.account_id THEN excluded.account_id
                ELSE eve_character.account_id
            END,
            name = excluded.name,
            corporation_id = excluded.corporation_id,
            alliance_id = excluded.alliance_id,
            esi_client_id = excluded.esi_client_id,
            encrypted_access_token = excluded.encrypted_access_token,
            encrypted_refresh_token = excluded.encrypted_refresh_token,
            esi_token_expires_at = excluded.esi_token_expires_at,
            updated_at = now()
        RETURNING id
        "#,
        resolved_account_id,
        eve_character_id,
        name,
        corporation_id,
        alliance_id,
        esi_client_id,
        encrypted_access.as_slice(),
        encrypted_refresh.as_slice(),
        expires_at,
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
    alliance_id: Option<i64>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO eve_character (eve_character_id, name, corporation_id, alliance_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        eve_character_id,
        name,
        corporation_id,
        alliance_id,
    )
    .fetch_one(pool)
    .await
    .context("failed to create orphan character")?;

    Ok(row.id)
}

pub async fn list_for_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<Character>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, account_id, eve_character_id, name, corporation_id, alliance_id,
               is_main, is_online, esi_client_id, esi_token_expires_at, created_at, updated_at
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
            alliance_id: r.alliance_id,
            is_main: r.is_main,
            is_online: r.is_online,
            esi_client_id: r.esi_client_id,
            esi_token_expires_at: r.esi_token_expires_at,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::accounts;

    fn test_key() -> Vec<u8> {
        vec![0u8; 32]
    }

    #[sqlx::test]
    async fn create_orphan_inserts_row(pool: PgPool) {
        let id = create_orphan(&pool, 12345, "Test Pilot", 1000001, None)
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
            None,
            "client1",
            "access_tok",
            "refresh_tok",
            chrono::Utc::now() + chrono::Duration::hours(1),
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
            None,
            "client1",
            "access_tok_v1",
            "refresh_tok_v1",
            chrono::Utc::now() + chrono::Duration::hours(1),
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
            None,
            "client1",
            "access_tok_v2",
            "refresh_tok_v2",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &test_key(),
        )
        .await
        .unwrap();
        tx2.commit().await.unwrap();

        assert_eq!(id1, id2);
        let chars = list_for_account(&pool, account_id).await.unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "Pilot Two Updated");
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
            None,
            "client1",
            "access",
            "refresh",
            chrono::Utc::now() + chrono::Duration::hours(1),
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
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &test_key(),
        )
        .await
        .unwrap();
        promote_if_no_main(&mut tx, account_id, char1).await.unwrap();
        let char2 = upsert_tokens(
            &mut tx,
            account_id,
            99005,
            "Second",
            1000001,
            None,
            "client1",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
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
        let id = create_orphan(&pool, 99006, "To Delete", 1000001, None)
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
            None,
            "c",
            "a",
            "r",
            chrono::Utc::now() + chrono::Duration::hours(1),
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
}
