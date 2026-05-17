use serde::Serialize;
use utoipa::ToSchema;

use crate::dto::{
    account::{CharacterDto, MeDto},
    keys::{CreatedKeyDto, KeyMetadataDto},
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
