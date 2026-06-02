use serde::Serialize;
use utoipa::ToSchema;

use crate::dto::{
    account::{CharacterDto, MeDto},
    acl::{AclDto, AclMemberDto},
    admin::{
        AdminAccountDto, AuditLogPageDto, BlockedCharacterDto, CharacterSearchResultDto,
        EsiCharacterSearchPageDto,
    },
    keys::{CreatedKeyDto, KeyMetadataDto},
    map::MapDto,
    preferences::PreferencesDto,
};

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn data(payload: T) -> Self {
        Self { data: payload }
    }
}

/// OpenAPI schema aliases — concrete instantiations of ApiResponse<T> for each response type.

#[derive(Serialize, ToSchema)]
pub struct MeResponse {
    pub data: MeDto,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedKeyResponse {
    pub data: CreatedKeyDto,
}

#[derive(Serialize, ToSchema)]
pub struct KeyListResponse {
    pub data: Vec<KeyMetadataDto>,
}

#[derive(Serialize, ToSchema)]
pub struct CharacterResponse {
    pub data: CharacterDto,
}

#[derive(Serialize, ToSchema)]
pub struct PreferencesResponse {
    pub data: PreferencesDto,
}

#[derive(Serialize, ToSchema)]
pub struct AdminAccountListResponse {
    pub data: Vec<AdminAccountDto>,
}

#[derive(Serialize, ToSchema)]
pub struct CharacterSearchResponse {
    pub data: Vec<CharacterSearchResultDto>,
}

#[derive(Serialize, ToSchema)]
pub struct EsiCharacterSearchResponse {
    pub data: EsiCharacterSearchPageDto,
}

#[derive(Serialize, ToSchema)]
pub struct BlockListResponse {
    pub data: Vec<BlockedCharacterDto>,
}

#[derive(Serialize, ToSchema)]
pub struct AuditLogPageResponse {
    pub data: AuditLogPageDto,
}

#[derive(Serialize, ToSchema)]
pub struct AclResponse {
    pub data: AclDto,
}

#[derive(Serialize, ToSchema)]
pub struct AclListResponse {
    pub data: Vec<AclDto>,
}

#[derive(Serialize, ToSchema)]
pub struct AclMemberResponse {
    pub data: AclMemberDto,
}

#[derive(Serialize, ToSchema)]
pub struct AclMemberListResponse {
    pub data: Vec<AclMemberDto>,
}

#[derive(Serialize, ToSchema)]
pub struct MapResponse {
    pub data: MapDto,
}

#[derive(Serialize, ToSchema)]
pub struct MapListResponse {
    pub data: Vec<MapDto>,
}
