use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::keys::{CreateKeyRequest, CreatedKeyDto, KeyMetadataDto},
    error::{AppError, ErrorEnvelope},
    handlers::middleware::AuthenticatedAccount,
    response::{ApiResponse, CreatedKeyResponse, KeyListResponse},
    services::api_keys as svc,
};

#[utoipa::path(
    post,
    path = "/api/v1/keys",
    request_body = CreateKeyRequest,
    responses(
        (status = 201, description = "Key created", body = CreatedKeyResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "keys",
)]
pub async fn create_key(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Json(body): Json<CreateKeyRequest>,
) -> Result<(StatusCode, Json<ApiResponse<CreatedKeyDto>>), AppError> {
    let name = svc::validate_name(&body.name)?;
    let created = svc::create_key(&state.db, account_id, name, body.expires_at).await?;

    let dto = CreatedKeyDto {
        id: created.id,
        key: created.plaintext,
        name: created.name,
        expires_at: created.expires_at,
        created_at: created.created_at,
    };
    Ok((StatusCode::CREATED, Json(ApiResponse::data(dto))))
}

#[utoipa::path(
    get,
    path = "/api/v1/keys",
    responses(
        (status = 200, description = "List of API keys", body = KeyListResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "keys",
)]
pub async fn list_keys(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
) -> Result<Json<ApiResponse<Vec<KeyMetadataDto>>>, AppError> {
    let keys = svc::list_keys(&state.db, account_id).await?;
    let dtos = keys.into_iter().map(KeyMetadataDto::from).collect();
    Ok(Json(ApiResponse::data(dtos)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/keys/{id}",
    params(("id" = Uuid, Path, description = "API key ID")),
    responses(
        (status = 204, description = "Key deleted"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 404, description = "Key not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "keys",
)]
pub async fn delete_key(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let deleted = svc::delete_key(&state.db, id, account_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}
