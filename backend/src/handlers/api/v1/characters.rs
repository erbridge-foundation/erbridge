use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::account::CharacterDto,
    error::{AppError, ErrorEnvelope},
    handlers::middleware::AuthenticatedAccount,
    response::{ApiResponse, CharacterResponse},
    services::account as svc,
};

#[utoipa::path(
    post,
    path = "/api/v1/characters/{id}/set-main",
    params(("id" = Uuid, Path, description = "Character ID")),
    responses(
        (status = 200, description = "Updated character", body = CharacterResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 404, description = "Character not found or not owned by caller", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "characters",
)]
pub async fn set_main(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(character_id): Path<Uuid>,
) -> Result<Json<ApiResponse<CharacterDto>>, AppError> {
    let character = svc::set_main_character(&state.db, account_id, character_id).await?;
    Ok(Json(ApiResponse::data(CharacterDto::from(character))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/characters/{id}",
    params(("id" = Uuid, Path, description = "Character ID")),
    responses(
        (status = 204, description = "Character deleted"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 404, description = "Character not found or not owned by caller", body = ErrorEnvelope),
        (status = 409, description = "Cannot remove — error.code is `cannot_remove_main` or `cannot_remove_last_character`", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "characters",
)]
pub async fn delete_character(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(character_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    svc::delete_character(&state.db, account_id, character_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
