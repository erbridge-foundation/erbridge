use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::map::{AttachAclRequest, CreateMapRequest, MapDto, UpdateMapRequest},
    error::{AppError, ErrorEnvelope},
    handlers::middleware::AuthenticatedAccount,
    response::{ApiResponse, MapListResponse, MapResponse},
    services::map as svc,
};

#[utoipa::path(
    get,
    path = "/api/v1/maps",
    responses(
        (status = 200, description = "Maps the account can read", body = MapListResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn list_maps(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
) -> Result<Json<ApiResponse<Vec<MapDto>>>, AppError> {
    let maps = svc::list_maps(&state.db, account_id).await?;
    let dtos = maps.into_iter().map(MapDto::from).collect();
    Ok(Json(ApiResponse::data(dtos)))
}

#[utoipa::path(
    post,
    path = "/api/v1/maps",
    request_body = CreateMapRequest,
    responses(
        (status = 201, description = "Map created", body = MapResponse),
        (status = 400, description = "Invalid map", body = ErrorEnvelope),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "ACL not owned", body = ErrorEnvelope),
        (status = 409, description = "Slug already taken", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn create_map(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Json(body): Json<CreateMapRequest>,
) -> Result<(StatusCode, Json<ApiResponse<MapDto>>), AppError> {
    let name = svc::validate_map_name(&body.name)?;
    let slug = svc::validate_slug(&body.slug)?;
    let description = svc::validate_description(body.description.as_deref())?;
    let map = svc::create_map(
        &state.db,
        account_id,
        name,
        slug,
        description,
        body.acl_id,
        body.default_acl.unwrap_or(false),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::data(MapDto::from(map))),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/maps/{map_id}",
    params(("map_id" = Uuid, Path, description = "Map ID")),
    responses(
        (status = 200, description = "Map detail", body = MapResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "No read access", body = ErrorEnvelope),
        (status = 404, description = "Map not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn get_map(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(map_id): Path<Uuid>,
) -> Result<Json<ApiResponse<MapDto>>, AppError> {
    let map = svc::get_map(&state.db, account_id, map_id).await?;
    Ok(Json(ApiResponse::data(MapDto::from(map))))
}

#[utoipa::path(
    get,
    path = "/api/v1/maps/by-slug/{slug}",
    params(("slug" = String, Path, description = "Map slug")),
    responses(
        (status = 200, description = "Map detail (with attached-ACL summaries)", body = MapResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 404, description = "Map not found or no read access", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn get_map_by_slug(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(slug): Path<String>,
) -> Result<Json<ApiResponse<MapDto>>, AppError> {
    let map = svc::get_map_by_slug(&state.db, account_id, &slug).await?;
    Ok(Json(ApiResponse::data(MapDto::from(map))))
}

#[utoipa::path(
    patch,
    path = "/api/v1/maps/{map_id}",
    params(("map_id" = Uuid, Path, description = "Map ID")),
    request_body = UpdateMapRequest,
    responses(
        (status = 200, description = "Map updated", body = MapResponse),
        (status = 400, description = "Invalid map", body = ErrorEnvelope),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Insufficient permission", body = ErrorEnvelope),
        (status = 404, description = "Map not found", body = ErrorEnvelope),
        (status = 409, description = "Slug already taken", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn update_map(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(map_id): Path<Uuid>,
    Json(body): Json<UpdateMapRequest>,
) -> Result<Json<ApiResponse<MapDto>>, AppError> {
    let name = svc::validate_map_name(&body.name)?;
    let slug = svc::validate_slug(&body.slug)?;
    let description = svc::validate_description(body.description.as_deref())?;
    let map = svc::update_map(&state.db, account_id, map_id, name, slug, description).await?;
    Ok(Json(ApiResponse::data(MapDto::from(map))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/maps/{map_id}",
    params(("map_id" = Uuid, Path, description = "Map ID")),
    responses(
        (status = 204, description = "Map deleted"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Insufficient permission", body = ErrorEnvelope),
        (status = 404, description = "Map not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn delete_map(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(map_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    svc::delete_map(&state.db, account_id, map_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/maps/{map_id}/acls",
    params(("map_id" = Uuid, Path, description = "Map ID")),
    request_body = AttachAclRequest,
    responses(
        (status = 204, description = "ACL attached"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Insufficient permission or ACL not owned", body = ErrorEnvelope),
        (status = 404, description = "Map or ACL not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn attach_acl(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(map_id): Path<Uuid>,
    Json(body): Json<AttachAclRequest>,
) -> Result<StatusCode, AppError> {
    svc::attach_acl_to_map(&state.db, account_id, map_id, body.acl_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/v1/maps/{map_id}/acls/{acl_id}",
    params(
        ("map_id" = Uuid, Path, description = "Map ID"),
        ("acl_id" = Uuid, Path, description = "ACL ID"),
    ),
    responses(
        (status = 204, description = "ACL detached"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Insufficient permission", body = ErrorEnvelope),
        (status = 404, description = "Attachment not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "maps",
)]
pub async fn detach_acl(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path((map_id, acl_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    svc::detach_acl_from_map(&state.db, account_id, map_id, acl_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
