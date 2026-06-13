use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::map::{Map, MapWithAcls};

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// A summary of an ACL attached to a map (only those the requester can manage).
#[derive(Serialize, ToSchema)]
pub struct AclSummaryDto {
    pub id: Uuid,
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct MapDto {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_account_id: Option<Uuid>,
    pub description: Option<String>,
    pub acls: Vec<AclSummaryDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Map> for MapDto {
    fn from(m: Map) -> Self {
        Self {
            id: m.id,
            name: m.name,
            slug: m.slug,
            owner_account_id: m.owner_account_id,
            description: m.description,
            acls: vec![],
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

impl From<MapWithAcls> for MapDto {
    fn from(m: MapWithAcls) -> Self {
        Self {
            id: m.id,
            name: m.name,
            slug: m.slug,
            owner_account_id: m.owner_account_id,
            description: m.description,
            acls: m
                .acls
                .into_iter()
                .map(|(id, name)| AclSummaryDto { id, name })
                .collect(),
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct CreateMapRequest {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    /// Optionally attach the map to an ACL the caller owns at creation time.
    /// Mutually exclusive with `default_acl`.
    pub acl_id: Option<Uuid>,
    /// When true, the backend mints a fresh ACL named after the map, seeds the
    /// caller's main character as an `admin` member (when a main exists),
    /// attaches it, and creates the map — all atomically. Mutually exclusive with
    /// `acl_id`.
    #[serde(default)]
    pub default_acl: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMapRequest {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct AttachAclRequest {
    pub acl_id: Uuid,
}
