use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::acl::Acl;
use crate::db::acl_member::AclMember;

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct AclDto {
    pub id: Uuid,
    pub name: String,
    pub owner_account_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Acl> for AclDto {
    fn from(a: Acl) -> Self {
        Self {
            id: a.id,
            name: a.name,
            owner_account_id: a.owner_account_id,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct AclMemberDto {
    pub id: Uuid,
    pub acl_id: Uuid,
    pub member_type: String,
    pub eve_entity_id: Option<i64>,
    pub character_id: Option<Uuid>,
    pub name: String,
    pub permission: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AclMember> for AclMemberDto {
    fn from(m: AclMember) -> Self {
        Self {
            id: m.id,
            acl_id: m.acl_id,
            member_type: m.member_type,
            eve_entity_id: m.eve_entity_id,
            character_id: m.character_id,
            name: m.name,
            permission: m.permission,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct AclNameRequest {
    pub name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AddMemberRequest {
    /// One of `character`, `corporation`, `alliance`.
    pub member_type: String,
    /// The member's durable EVE id — the EVE character/corporation/alliance id.
    /// Required for every member type (the picker has it from its ESI search).
    pub eve_entity_id: Option<i64>,
    /// Required for character members; the `eve_character.id` UUID (the internal
    /// FK link). `None` for corporation/alliance members.
    pub character_id: Option<Uuid>,
    /// Optional display-name snapshot.
    #[serde(default)]
    pub name: String,
    /// One of `read`, `read_write`, `manage`, `admin`, `deny`.
    pub permission: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMemberRequest {
    /// One of `read`, `read_write`, `manage`, `admin`, `deny`.
    pub permission: String,
}
