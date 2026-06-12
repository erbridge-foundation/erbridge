use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::acl::{AclDto, AclMemberDto, AclNameRequest, AddMemberRequest, UpdateMemberRequest},
    error::{AppError, ErrorEnvelope},
    handlers::middleware::AuthenticatedAccount,
    response::{
        AclListResponse, AclMemberListResponse, AclMemberResponse, AclResponse, ApiResponse,
    },
    services::acl::{self as svc, AddMemberInput},
};

#[utoipa::path(
    get,
    path = "/api/v1/acls",
    responses(
        (status = 200, description = "ACLs the account can manage", body = AclListResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn list_acls(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
) -> Result<Json<ApiResponse<Vec<AclDto>>>, AppError> {
    let acls = svc::list_manageable_for_account(&state.db, account_id).await?;
    let dtos = acls.into_iter().map(AclDto::from).collect();
    Ok(Json(ApiResponse::data(dtos)))
}

#[utoipa::path(
    post,
    path = "/api/v1/acls",
    request_body = AclNameRequest,
    responses(
        (status = 201, description = "ACL created", body = AclResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 400, description = "Invalid name", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn create_acl(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Json(body): Json<AclNameRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AclDto>>), AppError> {
    let name = svc::validate_acl_name(&body.name)?;
    let acl = svc::create_acl(&state.db, account_id, name).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::data(AclDto::from(acl))),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/acls/{acl_id}",
    params(("acl_id" = Uuid, Path, description = "ACL ID")),
    request_body = AclNameRequest,
    responses(
        (status = 200, description = "ACL renamed", body = AclResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Not the owner", body = ErrorEnvelope),
        (status = 404, description = "ACL not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn rename_acl(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(acl_id): Path<Uuid>,
    Json(body): Json<AclNameRequest>,
) -> Result<Json<ApiResponse<AclDto>>, AppError> {
    let name = svc::validate_acl_name(&body.name)?;
    let acl = svc::rename_acl(&state.db, account_id, acl_id, name).await?;
    Ok(Json(ApiResponse::data(AclDto::from(acl))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/acls/{acl_id}",
    params(("acl_id" = Uuid, Path, description = "ACL ID")),
    responses(
        (status = 204, description = "ACL deleted"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Not the owner", body = ErrorEnvelope),
        (status = 404, description = "ACL not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn delete_acl(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(acl_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    svc::delete_acl(&state.db, account_id, acl_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/acls/{acl_id}/members",
    params(("acl_id" = Uuid, Path, description = "ACL ID")),
    responses(
        (status = 200, description = "ACL members", body = AclMemberListResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Not the owner", body = ErrorEnvelope),
        (status = 404, description = "ACL not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn list_members(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(acl_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<AclMemberDto>>>, AppError> {
    let members = svc::list_members(&state.db, account_id, acl_id).await?;
    let dtos = members.into_iter().map(AclMemberDto::from).collect();
    Ok(Json(ApiResponse::data(dtos)))
}

#[utoipa::path(
    post,
    path = "/api/v1/acls/{acl_id}/members",
    params(("acl_id" = Uuid, Path, description = "ACL ID")),
    request_body = AddMemberRequest,
    responses(
        (status = 201, description = "Member added", body = AclMemberResponse),
        (status = 400, description = "Invalid member", body = ErrorEnvelope),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Not the owner", body = ErrorEnvelope),
        (status = 404, description = "ACL not found", body = ErrorEnvelope),
        (status = 409, description = "Entity is already a member (duplicate_acl_member)", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn add_member(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path(acl_id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AclMemberDto>>), AppError> {
    let member_type = svc::parse_member_type(&body.member_type)?;
    let permission = svc::parse_permission(&body.permission)?;
    let input = AddMemberInput {
        member_type,
        eve_entity_id: body.eve_entity_id,
        character_id: body.character_id,
        name: body.name,
        permission,
    };
    let member = svc::add_member(&state.db, account_id, acl_id, input).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::data(AclMemberDto::from(member))),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/acls/{acl_id}/members/{member_id}",
    params(
        ("acl_id" = Uuid, Path, description = "ACL ID"),
        ("member_id" = Uuid, Path, description = "Member ID"),
    ),
    request_body = UpdateMemberRequest,
    responses(
        (status = 200, description = "Member permission updated", body = AclMemberResponse),
        (status = 400, description = "Invalid permission", body = ErrorEnvelope),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Not the owner", body = ErrorEnvelope),
        (status = 404, description = "ACL or member not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn update_member(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path((acl_id, member_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateMemberRequest>,
) -> Result<Json<ApiResponse<AclMemberDto>>, AppError> {
    let permission = svc::parse_permission(&body.permission)?;
    let member =
        svc::update_member_permission(&state.db, account_id, acl_id, member_id, permission).await?;
    Ok(Json(ApiResponse::data(AclMemberDto::from(member))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/acls/{acl_id}/members/{member_id}",
    params(
        ("acl_id" = Uuid, Path, description = "ACL ID"),
        ("member_id" = Uuid, Path, description = "Member ID"),
    ),
    responses(
        (status = 204, description = "Member removed"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Not the owner", body = ErrorEnvelope),
        (status = 404, description = "ACL or member not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "acls",
)]
pub async fn remove_member(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Path((acl_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    svc::remove_member(&state.db, account_id, acl_id, member_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
