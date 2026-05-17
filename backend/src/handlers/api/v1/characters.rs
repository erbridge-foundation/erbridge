use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::account::CharacterDto,
    error::AppError,
    handlers::middleware::AuthenticatedAccount,
    response::ApiResponse,
    services::account as svc,
};

pub async fn set_main(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(character_id): Path<Uuid>,
) -> Result<Json<ApiResponse<CharacterDto>>, AppError> {
    let character = svc::set_main_character(&state.db, &state.http_client, account_id, character_id).await?;
    Ok(Json(ApiResponse::data(CharacterDto::from(character))))
}

pub async fn delete_character(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(character_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    svc::delete_character(&state.db, account_id, character_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
