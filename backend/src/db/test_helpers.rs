//! Shared test fixtures for the map/acl db-layer tests.
//!
//! These insert minimal valid rows directly (bypassing the token-encryption
//! path of `characters::upsert_tokens`) so resolver/listing tests can cheaply
//! set a character's corporation and alliance.

#![cfg(test)]

use sqlx::PgPool;
use uuid::Uuid;

/// Inserts a character on `account_id` with corp `1_000_001` and no alliance.
/// Returns the `eve_character.id` (UUID PK).
pub async fn insert_character(
    pool: &PgPool,
    account_id: Uuid,
    eve_character_id: i64,
    name: &str,
) -> Uuid {
    insert_character_full(pool, account_id, eve_character_id, name, 1_000_001, None).await
}

/// Inserts a character on `account_id` with the given corporation and optional
/// alliance. Returns the `eve_character.id` (UUID PK).
pub async fn insert_character_full(
    pool: &PgPool,
    account_id: Uuid,
    eve_character_id: i64,
    name: &str,
    corporation_id: i64,
    alliance_id: Option<i64>,
) -> Uuid {
    let row = sqlx::query!(
        r#"
        INSERT INTO eve_character (
            account_id, eve_character_id, name, corporation_id, corporation_name, alliance_id
        ) VALUES ($1, $2, $3, $4, 'Test Corp', $5)
        RETURNING id
        "#,
        account_id,
        eve_character_id,
        name,
        corporation_id,
        alliance_id,
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert test character");
    row.id
}
