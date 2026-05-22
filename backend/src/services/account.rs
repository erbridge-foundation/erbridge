use sqlx::PgPool;
use uuid::Uuid;

use crate::{
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

fn derive_token_status(has_refresh_token: bool) -> TokenStatus {
    if has_refresh_token {
        TokenStatus::Active
    } else {
        TokenStatus::Expired
    }
}

fn character_to_info(c: characters::Character) -> CharacterInfo {
    let portrait_url = format!(
        "https://images.evetech.net/characters/{}/portrait?size=128",
        c.eve_character_id
    );
    let token_status = derive_token_status(c.encrypted_refresh_token.is_some());
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
    // Verify ownership.
    let info = characters::is_main(pool, character_id)
        .await
        .map_err(AppError::Internal)?;

    match info {
        Some((owner_id, _)) if owner_id == account_id => {}
        _ => return Err(AppError::NotFound),
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
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

    accounts::soft_delete(pool, account_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(())
}

pub async fn delete_character(
    pool: &PgPool,
    account_id: Uuid,
    character_id: Uuid,
) -> Result<(), AppError> {
    let info = characters::is_main(pool, character_id)
        .await
        .map_err(AppError::Internal)?;

    let (owner_id, is_main) = match info {
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

    characters::delete_character(pool, character_id)
        .await
        .map_err(AppError::Internal)?;

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
    fn token_status_active_when_has_refresh_token() {
        assert_eq!(derive_token_status(true), TokenStatus::Active);
    }

    #[test]
    fn token_status_expired_when_no_refresh_token() {
        assert_eq!(derive_token_status(false), TokenStatus::Expired);
    }

    #[sqlx::test]
    async fn delete_account_blocks_last_server_admin(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let admin_id = accounts::resolve_or_create(&mut tx, None, 1001)
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
        let first = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let second = accounts::resolve_or_create(&mut tx, None, 1002)
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

        delete_account(&pool, first).await.unwrap();

        let account = accounts::get_account(&pool, first).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
    }

    #[sqlx::test]
    async fn delete_account_allows_non_admin(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let _admin = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let user = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        delete_account(&pool, user).await.unwrap();

        let account = accounts::get_account(&pool, user).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
    }
}
