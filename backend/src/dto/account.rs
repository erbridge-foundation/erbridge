use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{db::accounts::Account, services::account::CharacterInfo};

#[derive(Serialize, ToSchema)]
pub struct AccountDto {
    pub id: Uuid,
    pub status: String,
    pub is_server_admin: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Account> for AccountDto {
    fn from(a: Account) -> Self {
        Self {
            id: a.id,
            status: a.status,
            is_server_admin: a.is_server_admin,
            created_at: a.created_at,
        }
    }
}

#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    Active,
    Expired,
}

#[derive(Serialize, ToSchema)]
pub struct CharacterDto {
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

impl From<CharacterInfo> for CharacterDto {
    fn from(c: CharacterInfo) -> Self {
        Self {
            id: c.id,
            eve_character_id: c.eve_character_id,
            name: c.name,
            corporation_id: c.corporation_id,
            corporation_name: c.corporation_name,
            alliance_id: c.alliance_id,
            alliance_name: c.alliance_name,
            is_main: c.is_main,
            portrait_url: c.portrait_url,
            token_status: c.token_status,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct MeDto {
    pub account: AccountDto,
    pub characters: Vec<CharacterDto>,
}
