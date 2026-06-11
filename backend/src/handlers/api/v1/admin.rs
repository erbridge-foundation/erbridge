use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::admin::{
        AdminAccountDto, AuditLogEntryDto, AuditLogPageDto, BlockCharacterRequest,
        BlockedCharacterDto, CharacterSearchResultDto, EsiCharacterSearchPageDto,
        EsiCharacterSearchResultDto,
    },
    error::{AppError, ErrorEnvelope},
    esi::{public_info, search},
    handlers::middleware::AdminAccount,
    response::{
        AdminAccountListResponse, ApiResponse, AuditLogPageResponse, BlockListResponse,
        CharacterSearchResponse, EsiCharacterSearchResponse,
    },
    services::admin::{self as svc, EsiSearchContext, EsiSearchOutcome},
};

#[utoipa::path(
    get,
    path = "/api/v1/admin/accounts",
    responses(
        (status = 200, description = "All accounts with their characters", body = AdminAccountListResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn list_accounts(
    State(state): State<AppState>,
    _admin: AdminAccount,
) -> Result<Json<ApiResponse<Vec<AdminAccountDto>>>, AppError> {
    let accounts = svc::list_accounts(&state.db).await?;
    let dtos = accounts.into_iter().map(AdminAccountDto::from).collect();
    Ok(Json(ApiResponse::data(dtos)))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/characters/search",
    params(
        ("q" = String, Query, description = "Case-insensitive name fragment"),
        ("limit" = Option<i64>, Query, description = "Max results (clamped)"),
    ),
    responses(
        (status = 200, description = "Matching characters with their owning account", body = CharacterSearchResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn search_characters(
    State(state): State<AppState>,
    _admin: AdminAccount,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<CharacterSearchResultDto>>>, AppError> {
    let results = svc::search_characters(&state.db, &query.q, query.limit).await?;
    let dtos = results
        .into_iter()
        .map(CharacterSearchResultDto::from)
        .collect();
    Ok(Json(ApiResponse::data(dtos)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/characters/esi-search",
    params(
        ("q" = String, Query, description = "Case-insensitive name fragment (min 3 chars)"),
        ("limit" = Option<i64>, Query, description = "Max results (clamped)"),
    ),
    responses(
        (status = 200, description = "Matching ESI characters, or an unavailable indicator", body = EsiCharacterSearchResponse),
        (status = 400, description = "Fragment shorter than 3 characters", body = ErrorEnvelope),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn esi_search_characters(
    State(state): State<AppState>,
    AdminAccount(admin_id): AdminAccount,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<EsiCharacterSearchPageDto>>, AppError> {
    // Enforce ESI's minimum fragment length before any token/ESI work.
    if query.q.chars().count() < search::MIN_SEARCH_LEN {
        return Err(AppError::BadRequest(format!(
            "search term must be at least {} characters",
            search::MIN_SEARCH_LEN
        )));
    }

    let encryption_key = crate::crypto::token_encryption_key(&state.config.encryption_secret)
        .map_err(AppError::Internal)?;

    let ctx = EsiSearchContext {
        http: &state.http_client,
        esi_base_url: public_info::ESI_BASE,
        token_endpoint: &state.esi_metadata.token_endpoint,
        client_id: &state.config.esi_client_id,
        client_secret: &state.config.esi_client_secret,
        encryption_key: &encryption_key,
    };

    let page =
        match svc::esi_search_characters(&state.db, ctx, admin_id, &query.q, query.limit).await? {
            EsiSearchOutcome::Available(results) => EsiCharacterSearchPageDto {
                results: results
                    .into_iter()
                    .map(EsiCharacterSearchResultDto::from)
                    .collect(),
                unavailable: false,
            },
            EsiSearchOutcome::Unavailable => EsiCharacterSearchPageDto {
                results: Vec::new(),
                unavailable: true,
            },
        };

    Ok(Json(ApiResponse::data(page)))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/accounts/{id}/grant-admin",
    params(("id" = Uuid, Path, description = "Target account id")),
    responses(
        (status = 204, description = "Granted (or already an admin)"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
        (status = 404, description = "Account not found", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn grant_admin(
    State(state): State<AppState>,
    AdminAccount(admin_id): AdminAccount,
    Path(target): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    svc::grant_admin(&state.db, admin_id, target).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/accounts/{id}/revoke-admin",
    params(("id" = Uuid, Path, description = "Target account id")),
    responses(
        (status = 204, description = "Revoked (or already not an admin)"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
        (status = 404, description = "Account not found", body = ErrorEnvelope),
        (status = 409, description = "Cannot remove the last server admin", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn revoke_admin(
    State(state): State<AppState>,
    AdminAccount(admin_id): AdminAccount,
    Path(target): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    svc::revoke_admin(&state.db, admin_id, target).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/blocks",
    responses(
        (status = 200, description = "All blocked characters", body = BlockListResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn list_blocks(
    State(state): State<AppState>,
    _admin: AdminAccount,
) -> Result<Json<ApiResponse<Vec<BlockedCharacterDto>>>, AppError> {
    let blocks = svc::list_blocks(&state.db).await?;
    let dtos = blocks.into_iter().map(BlockedCharacterDto::from).collect();
    Ok(Json(ApiResponse::data(dtos)))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/blocks",
    request_body = BlockCharacterRequest,
    responses(
        (status = 204, description = "Blocked (or already blocked)"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
        (status = 409, description = "Cannot block your own character", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn block_character(
    State(state): State<AppState>,
    AdminAccount(admin_id): AdminAccount,
    Json(body): Json<BlockCharacterRequest>,
) -> Result<StatusCode, AppError> {
    // Best-effort ESI snapshot — a block SHALL succeed even when ESI is down, so
    // a failed fetch leaves name/corp NULL (the helper never errors).
    let (character_name, corporation_name) =
        public_info::fetch_character_block_snapshot(&state.http_client, body.eve_character_id)
            .await;

    svc::block_character(
        &state.db,
        admin_id,
        body.eve_character_id,
        body.reason.as_deref(),
        character_name.as_deref(),
        corporation_name.as_deref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/blocks/{eve_character_id}",
    params(("eve_character_id" = i64, Path, description = "EVE character id to unblock")),
    responses(
        (status = 204, description = "Unblocked"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
        (status = 404, description = "Character is not blocked", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn unblock_character(
    State(state): State<AppState>,
    AdminAccount(admin_id): AdminAccount,
    Path(eve_character_id): Path<i64>,
) -> Result<StatusCode, AppError> {
    svc::unblock_character(&state.db, admin_id, eve_character_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct AuditQuery {
    pub event_type: Option<String>,
    pub actor: Option<Uuid>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/audit",
    params(
        ("event_type" = Option<String>, Query, description = "Filter by event type"),
        ("actor" = Option<Uuid>, Query, description = "Filter by actor account id"),
        ("target_type" = Option<String>, Query, description = "Filter by target type"),
        ("target_id" = Option<String>, Query, description = "Filter by target id"),
        ("target_name" = Option<String>, Query, description = "Filter by target name (case-insensitive)"),
        ("before" = Option<String>, Query, description = "Keyset cursor (RFC 3339); returns entries older than this"),
        ("limit" = Option<i64>, Query, description = "Max entries (clamped)"),
    ),
    responses(
        (status = 200, description = "Audit-log page with next cursor", body = AuditLogPageResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 403, description = "Server admin required", body = ErrorEnvelope),
    ),
    security(("session_cookie" = [])),
    tag = "admin",
)]
pub async fn list_audit(
    State(state): State<AppState>,
    _admin: AdminAccount,
    Query(query): Query<AuditQuery>,
) -> Result<Json<ApiResponse<AuditLogPageDto>>, AppError> {
    let entries = svc::list_audit_log(
        &state.db,
        query.event_type.as_deref(),
        query.actor,
        query.target_type.as_deref(),
        query.target_id.as_deref(),
        query.target_name.as_deref(),
        query.before,
        query.limit,
    )
    .await?;

    // The next-page cursor is the oldest returned entry's occurred_at (entries
    // are newest-first, so the last one is the oldest).
    let next_before = entries.last().map(|e| e.occurred_at);
    let dtos = entries.into_iter().map(AuditLogEntryDto::from).collect();

    Ok(Json(ApiResponse::data(AuditLogPageDto {
        entries: dtos,
        next_before,
    })))
}
