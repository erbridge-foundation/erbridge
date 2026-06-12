use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent},
    db::{accounts, characters},
    dto::account::TokenStatus,
    error::{AppError, ConflictKind},
};

pub struct CharacterInfo {
    pub id: Uuid,
    pub eve_character_id: i64,
    pub name: String,
    pub corporation_id: i64,
    pub corporation_name: String,
    pub alliance_id: Option<i64>,
    pub alliance_name: Option<String>,
    pub is_main: bool,
    pub portrait_url: String,
    pub token_status: TokenStatus,
}

fn character_to_info(c: characters::Character) -> CharacterInfo {
    let portrait_url = format!(
        "https://images.evetech.net/characters/{}/portrait?size=128",
        c.eve_character_id
    );
    let token_status = TokenStatus::from_db(&c.token_status);
    CharacterInfo {
        id: c.id,
        eve_character_id: c.eve_character_id,
        name: c.name,
        corporation_id: c.corporation_id,
        corporation_name: c.corporation_name,
        alliance_id: c.alliance_id,
        alliance_name: c.alliance_name,
        is_main: c.is_main,
        portrait_url,
        token_status,
    }
}

pub struct MeInfo {
    pub account: accounts::Account,
    pub characters: Vec<CharacterInfo>,
}

pub async fn get_me(pool: &PgPool, account_id: Uuid) -> Result<MeInfo, AppError> {
    let account = accounts::get_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    let chars = characters::list_for_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(MeInfo {
        account,
        characters: chars.into_iter().map(character_to_info).collect(),
    })
}

pub async fn set_main_character(
    pool: &PgPool,
    account_id: Uuid,
    character_id: Uuid,
) -> Result<CharacterInfo, AppError> {
    // Verify ownership and capture the new main's EVE ID + name for the audit
    // payload (snapshotted so the row stays readable after a later rename/delete).
    let (new_main_eve_id, new_main_name) = match characters::lookup_for_account(pool, character_id)
        .await
        .map_err(AppError::Internal)?
    {
        Some((owner_id, eve_id, name, _)) if owner_id == account_id => (eve_id, name),
        _ => return Err(AppError::NotFound),
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    // Emit the audit row *before* the is_main flip so the actor-character
    // snapshot resolves to the outgoing main.
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::CharacterSetMain {
            account_id,
            eve_character_id: new_main_eve_id,
            character_name: new_main_name,
        },
    )
    .await
    .map_err(AppError::Internal)?;
    characters::set_main(&mut tx, account_id, character_id)
        .await
        .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Reload the updated character from DB — no ESI call needed.
    let chars = characters::list_for_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?;

    let c = chars
        .into_iter()
        .find(|c| c.id == character_id)
        .ok_or(AppError::NotFound)?;

    Ok(character_to_info(c))
}

pub async fn delete_account(pool: &PgPool, account_id: Uuid) -> Result<(), AppError> {
    let account = accounts::get_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    if account.is_server_admin {
        let admin_count = accounts::count_server_admins(pool)
            .await
            .map_err(AppError::Internal)?;
        if admin_count <= 1 {
            return Err(AppError::Conflict(
                ConflictKind::CannotRemoveLastServerAdmin,
            ));
        }
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    // Emit the audit row before the state change so the main lookup still
    // resolves the outgoing main's snapshot — the soft-delete itself does not
    // touch characters, but the discipline matches set-main below.
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AccountDeletionRequested { account_id },
    )
    .await
    .map_err(AppError::Internal)?;
    accounts::soft_delete(&mut tx, account_id)
        .await
        .map_err(AppError::Internal)?;
    characters::clear_tokens_for_account(&mut tx, account_id)
        .await
        .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(())
}

pub async fn delete_character(
    pool: &PgPool,
    account_id: Uuid,
    character_id: Uuid,
) -> Result<(), AppError> {
    let (owner_id, eve_character_id, character_name, is_main) =
        match characters::lookup_for_account(pool, character_id)
            .await
            .map_err(AppError::Internal)?
        {
            Some(v) => v,
            None => return Err(AppError::NotFound),
        };

    if owner_id != account_id {
        return Err(AppError::NotFound);
    }

    let count = characters::count_for_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?;

    if count <= 1 {
        return Err(AppError::Conflict(ConflictKind::CannotRemoveLastCharacter));
    }

    if is_main {
        return Err(AppError::Conflict(ConflictKind::CannotRemoveMain));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::CharacterRemoved {
            account_id,
            eve_character_id,
            character_name,
        },
    )
    .await
    .map_err(AppError::Internal)?;
    characters::delete_character_in_tx(&mut tx, character_id)
        .await
        .map_err(AppError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_url_format() {
        let url = format!(
            "https://images.evetech.net/characters/{}/portrait?size=128",
            12345i64
        );
        assert_eq!(
            url,
            "https://images.evetech.net/characters/12345/portrait?size=128"
        );
    }

    #[test]
    fn token_status_maps_db_values() {
        assert_eq!(TokenStatus::from_db("valid"), TokenStatus::Active);
        assert_eq!(TokenStatus::from_db("token_expired"), TokenStatus::Expired);
        assert_eq!(
            TokenStatus::from_db("owner_mismatch"),
            TokenStatus::OwnerMismatch
        );
        // Unknown values fail safe to Expired.
        assert_eq!(TokenStatus::from_db("bogus"), TokenStatus::Expired);
    }

    #[sqlx::test]
    async fn delete_account_blocks_last_server_admin(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (admin_id, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let err = delete_account(&pool, admin_id).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotRemoveLastServerAdmin)
        ));

        let account = accounts::get_account(&pool, admin_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.status, "active");
    }

    #[sqlx::test]
    async fn delete_account_allows_admin_when_another_admin_exists(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (first, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (second, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        sqlx::query!(
            "UPDATE account SET is_server_admin = TRUE WHERE id = $1",
            second
        )
        .execute(&pool)
        .await
        .unwrap();

        let char_id = insert_test_character_with_tokens(&pool, first, 1001).await;

        delete_account(&pool, first).await.unwrap();

        let account = accounts::get_account(&pool, first).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert_character_tokens_cleared(&pool, char_id).await;
    }

    #[sqlx::test]
    async fn delete_account_allows_non_admin(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (_admin, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (user, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let char_id = insert_test_character_with_tokens(&pool, user, 1002).await;

        delete_account(&pool, user).await.unwrap();

        let account = accounts::get_account(&pool, user).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert_character_tokens_cleared(&pool, char_id).await;
    }

    #[sqlx::test]
    async fn delete_account_is_atomic_on_account_with_characters(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (_admin, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (user, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let char_one = insert_test_character_with_tokens(&pool, user, 9001).await;
        let char_two = insert_test_character_with_tokens(&pool, user, 9002).await;

        delete_account(&pool, user).await.unwrap();

        let account = accounts::get_account(&pool, user).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert!(account.delete_requested_at.is_some());
        assert_character_tokens_cleared(&pool, char_one).await;
        assert_character_tokens_cleared(&pool, char_two).await;
    }

    async fn insert_test_character_with_tokens(
        pool: &PgPool,
        account_id: Uuid,
        eve_character_id: i64,
    ) -> Uuid {
        let row = sqlx::query!(
            r#"
            INSERT INTO eve_character (
                account_id, eve_character_id, name, corporation_id, corporation_name,
                esi_client_id, encrypted_access_token, encrypted_refresh_token,
                access_token_expires_at, scopes
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
            account_id,
            eve_character_id,
            format!("Pilot {eve_character_id}"),
            1_000_001_i64,
            "Test Corp",
            "test-client",
            &[1u8, 2, 3][..],
            &[4u8, 5, 6][..],
            chrono::Utc::now() + chrono::Duration::hours(1),
            &["esi-skills.read_skills.v1".to_string()][..],
        )
        .fetch_one(pool)
        .await
        .unwrap();
        row.id
    }

    async fn assert_character_tokens_cleared(pool: &PgPool, character_id: Uuid) {
        let row = sqlx::query!(
            r#"
            SELECT encrypted_access_token, encrypted_refresh_token,
                   access_token_expires_at, scopes
            FROM eve_character WHERE id = $1
            "#,
            character_id
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(row.encrypted_access_token.is_none());
        assert!(row.encrypted_refresh_token.is_none());
        assert!(row.access_token_expires_at.is_none());
        assert!(row.scopes.is_empty());
    }
}
