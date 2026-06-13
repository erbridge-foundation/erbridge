use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::services::entity_search::{CharacterMatch, EntityMatch, EntitySearchResults};

/// A character match, carrying the `eve_character.id` UUID an `acl_member`
/// references **only when a local row already exists** (account-owned or orphan),
/// plus the numeric `eve_character_id` and current name. When the character is
/// unknown, `id` is null — the search mints nothing, and the orphan is minted at
/// the ACL member add (which accepts `eve_entity_id` with no `character_id`).
#[derive(Serialize, ToSchema)]
pub struct EntityCharacterDto {
    /// The internal `eve_character.id` UUID — the value an `acl_member` stores.
    /// Null when no local row exists for the character yet.
    pub id: Option<Uuid>,
    pub eve_character_id: i64,
    pub name: String,
}

impl From<CharacterMatch> for EntityCharacterDto {
    fn from(c: CharacterMatch) -> Self {
        Self {
            id: c.id,
            eve_character_id: c.eve_character_id,
            name: c.name,
        }
    }
}

/// A corporation or alliance match, carrying the numeric `eve_entity_id` an
/// `acl_member` stores for those member types plus the current name.
#[derive(Serialize, ToSchema)]
pub struct EntityOrgDto {
    pub eve_entity_id: i64,
    pub name: String,
}

impl From<EntityMatch> for EntityOrgDto {
    fn from(e: EntityMatch) -> Self {
        Self {
            eve_entity_id: e.eve_entity_id,
            name: e.name,
        }
    }
}

/// The grouped entity-search result. `unavailable` is `true` when the search
/// could not be performed (the account has no usable token, the token lacks the
/// search scope, or ESI is down); in that case the group lists are empty and the
/// UI shows a "search unavailable" notice rather than "no matches".
#[derive(Serialize, ToSchema)]
pub struct EntitySearchPageDto {
    pub characters: Vec<EntityCharacterDto>,
    pub corporations: Vec<EntityOrgDto>,
    pub alliances: Vec<EntityOrgDto>,
    pub unavailable: bool,
}

impl EntitySearchPageDto {
    /// The graceful "could not search" page: empty groups + `unavailable: true`.
    pub fn unavailable() -> Self {
        Self {
            characters: Vec::new(),
            corporations: Vec::new(),
            alliances: Vec::new(),
            unavailable: true,
        }
    }
}

impl From<EntitySearchResults> for EntitySearchPageDto {
    fn from(r: EntitySearchResults) -> Self {
        Self {
            characters: r
                .characters
                .into_iter()
                .map(EntityCharacterDto::from)
                .collect(),
            corporations: r.corporations.into_iter().map(EntityOrgDto::from).collect(),
            alliances: r.alliances.into_iter().map(EntityOrgDto::from).collect(),
            unavailable: false,
        }
    }
}
