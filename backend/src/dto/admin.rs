use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    audit::AuditLogEntry,
    db::blocks::BlockedEveCharacter,
    dto::account::TokenStatus,
    services::admin::{AdminAccountInfo, AdminCharacterSearchResult, EsiCharacterSearchResult},
};

// ── accounts list ──────────────────────────────────────────────────────────────

/// A character as it appears in the admin accounts list — enough to identify an
/// account by its pilots, without the token/credential fields of the full
/// `CharacterDto`.
#[derive(Serialize, ToSchema)]
pub struct AdminAccountCharacterDto {
    pub eve_character_id: i64,
    pub name: String,
    pub is_main: bool,
    /// Token health, so an admin can spot a transferred (`owner_mismatch`) or
    /// expired character on the account.
    pub token_status: TokenStatus,
}

#[derive(Serialize, ToSchema)]
pub struct AdminAccountDto {
    pub id: Uuid,
    pub status: String,
    pub is_server_admin: bool,
    pub created_at: DateTime<Utc>,
    pub characters: Vec<AdminAccountCharacterDto>,
}

impl From<AdminAccountInfo> for AdminAccountDto {
    fn from(a: AdminAccountInfo) -> Self {
        Self {
            id: a.account.id,
            status: a.account.status,
            is_server_admin: a.account.is_server_admin,
            created_at: a.account.created_at,
            characters: a
                .characters
                .into_iter()
                .map(
                    |(eve_character_id, name, is_main, token_status)| AdminAccountCharacterDto {
                        eve_character_id,
                        name,
                        is_main,
                        token_status: TokenStatus::from_db(&token_status),
                    },
                )
                .collect(),
        }
    }
}

// ── character search ─────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct CharacterSearchResultDto {
    pub eve_character_id: i64,
    pub name: String,
    pub is_main: bool,
    /// `None` for an orphan character (no owning account).
    pub account_id: Option<Uuid>,
    /// Deterministic ESI portrait image URL.
    pub portrait_url: String,
    /// Whether this character is already in the block list.
    pub already_blocked: bool,
}

impl From<AdminCharacterSearchResult> for CharacterSearchResultDto {
    fn from(c: AdminCharacterSearchResult) -> Self {
        Self {
            eve_character_id: c.eve_character_id,
            name: c.name,
            is_main: c.is_main,
            account_id: c.account_id,
            portrait_url: c.portrait_url,
            already_blocked: c.already_blocked,
        }
    }
}

/// A character matched via ESI (no local account context).
#[derive(Serialize, ToSchema)]
pub struct EsiCharacterSearchResultDto {
    pub eve_character_id: i64,
    pub name: String,
    pub portrait_url: String,
    pub already_blocked: bool,
}

impl From<EsiCharacterSearchResult> for EsiCharacterSearchResultDto {
    fn from(c: EsiCharacterSearchResult) -> Self {
        Self {
            eve_character_id: c.eve_character_id,
            name: c.name,
            portrait_url: c.portrait_url,
            already_blocked: c.already_blocked,
        }
    }
}

/// Page wrapper for the ESI character search. `unavailable` is `true` when the
/// search could not be performed (the admin's token lacks the search scope, is
/// unrefreshable, or ESI is down); in that case `results` is empty and the UI
/// shows an "ESI search unavailable" notice rather than "no matches".
#[derive(Serialize, ToSchema)]
pub struct EsiCharacterSearchPageDto {
    pub results: Vec<EsiCharacterSearchResultDto>,
    pub unavailable: bool,
}

// ── blocks ─────────────────────────────────────────────────────────────────────

/// Request body for `POST /api/v1/admin/blocks`.
#[derive(Deserialize, ToSchema)]
pub struct BlockCharacterRequest {
    pub eve_character_id: i64,
    pub reason: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct BlockedCharacterDto {
    pub eve_character_id: i64,
    pub character_name: Option<String>,
    pub corporation_name: Option<String>,
    pub reason: Option<String>,
    pub blocked_by: Option<Uuid>,
    pub blocked_at: DateTime<Utc>,
}

impl From<BlockedEveCharacter> for BlockedCharacterDto {
    fn from(b: BlockedEveCharacter) -> Self {
        Self {
            eve_character_id: b.eve_character_id,
            character_name: b.character_name,
            corporation_name: b.corporation_name,
            reason: b.reason,
            blocked_by: b.blocked_by,
            blocked_at: b.blocked_at,
        }
    }
}

// ── audit log ──────────────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct AuditLogEntryDto {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub actor_account_id: Option<Uuid>,
    pub actor_character_id: Option<i64>,
    pub actor_character_name: Option<String>,
    pub event_type: String,
    pub details: serde_json::Value,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
}

impl From<AuditLogEntry> for AuditLogEntryDto {
    fn from(e: AuditLogEntry) -> Self {
        Self {
            id: e.id,
            occurred_at: e.occurred_at,
            actor_account_id: e.actor_account_id,
            actor_character_id: e.actor_character_id,
            actor_character_name: e.actor_character_name,
            event_type: e.event_type,
            details: e.details,
            target_type: e.target_type,
            target_id: e.target_id,
            target_name: e.target_name,
        }
    }
}

/// A page of audit-log entries with the keyset cursor for the next (older) page.
#[derive(Serialize, ToSchema)]
pub struct AuditLogPageDto {
    pub entries: Vec<AuditLogEntryDto>,
    /// `occurred_at` of the oldest returned entry — pass as `before` to fetch
    /// the next page. `None` when the page is empty.
    pub next_before: Option<DateTime<Utc>>,
}
