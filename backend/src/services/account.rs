use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::{accounts, characters},
    error::AppError,
    esi::public_info,
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
}

pub struct MeInfo {
    pub account: accounts::Account,
    pub characters: Vec<CharacterInfo>,
}

pub async fn get_me(
    pool: &PgPool,
    http: &reqwest::Client,
    account_id: Uuid,
) -> Result<MeInfo, AppError> {
    let account = accounts::get_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    let chars = characters::list_for_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?;

    let mut character_infos = Vec::with_capacity(chars.len());
    for c in chars {
        let corporation_name = public_info::fetch_corporation_name(http, c.corporation_id)
            .await
            .map_err(AppError::Internal)?;

        let alliance_name = match c.alliance_id {
            Some(id) => Some(
                public_info::fetch_alliance_name(http, id)
                    .await
                    .map_err(AppError::Internal)?,
            ),
            None => None,
        };

        let portrait_url = format!(
            "https://images.evetech.net/characters/{}/portrait?size=128",
            c.eve_character_id
        );

        character_infos.push(CharacterInfo {
            id: c.id,
            eve_character_id: c.eve_character_id,
            name: c.name,
            corporation_id: c.corporation_id,
            corporation_name,
            alliance_id: c.alliance_id,
            alliance_name,
            is_main: c.is_main,
            portrait_url,
        });
    }

    Ok(MeInfo {
        account,
        characters: character_infos,
    })
}

pub async fn set_main_character(
    pool: &PgPool,
    http: &reqwest::Client,
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

    // Reload the updated character.
    let chars = characters::list_for_account(pool, account_id)
        .await
        .map_err(AppError::Internal)?;

    let c = chars
        .into_iter()
        .find(|c| c.id == character_id)
        .ok_or(AppError::NotFound)?;

    let corporation_name = public_info::fetch_corporation_name(http, c.corporation_id)
        .await
        .map_err(AppError::Internal)?;

    let alliance_name = match c.alliance_id {
        Some(id) => Some(
            public_info::fetch_alliance_name(http, id)
                .await
                .map_err(AppError::Internal)?,
        ),
        None => None,
    };

    let portrait_url = format!(
        "https://images.evetech.net/characters/{}/portrait?size=128",
        c.eve_character_id
    );

    Ok(CharacterInfo {
        id: c.id,
        eve_character_id: c.eve_character_id,
        name: c.name,
        corporation_id: c.corporation_id,
        corporation_name,
        alliance_id: c.alliance_id,
        alliance_name,
        is_main: c.is_main,
        portrait_url,
    })
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
        return Err(AppError::Conflict(
            "cannot_remove_last_character".to_string(),
        ));
    }

    if is_main {
        return Err(AppError::Conflict("cannot_remove_main".to_string()));
    }

    characters::delete_character(pool, character_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(())
}

#[cfg(test)]
mod tests {
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
}
