use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::api_keys::ApiKeyMetadata;

#[derive(Deserialize, ToSchema)]
pub struct CreateKeyRequest {
    pub name: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Returned only on key creation — includes the plaintext key.
#[derive(Serialize, ToSchema)]
pub struct CreatedKeyDto {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Returned from list endpoints — no plaintext key.
#[derive(Serialize, ToSchema)]
pub struct KeyMetadataDto {
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<ApiKeyMetadata> for KeyMetadataDto {
    fn from(m: ApiKeyMetadata) -> Self {
        Self {
            id: m.id,
            name: m.name,
            scope: m.scope,
            expires_at: m.expires_at,
            created_at: m.created_at,
        }
    }
}
