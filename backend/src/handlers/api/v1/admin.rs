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
        jwks: &state.jwks,
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
    /// Combined name search: matches the actor character name OR the target
    /// name as a case-insensitive substring.
    pub q: Option<String>,
    /// Tiered relative time window mapped to a day-snapped `since` lower bound:
    /// `7d` (default), `30d`, `90d`, `365d`, or a per-year bucket `year:YYYY`.
    pub window: Option<String>,
    /// Explicit lower time bound (RFC 3339); takes precedence over `window`.
    pub since: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

/// Maps the optional `window` tier (and the default when absent) to a
/// day-snapped `since` lower bound. Relative tiers subtract a whole number of
/// days from the start of the current UTC day, so the predicate is stable
/// within a day and the query is cacheable. A `year:YYYY` bucket snaps to the
/// start of that calendar year. The deepest selectable *relative* tier is one
/// year (`365d`); unrecognised values fall back to the 7-day default. An
/// explicit `since` always wins over `window`.
fn resolve_since(window: Option<&str>, since: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    use chrono::{Duration, TimeZone};

    if let Some(s) = since {
        return Some(s);
    }

    // Start of the current UTC day — the snap anchor for relative tiers.
    let day_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|naive| Utc.from_utc_datetime(&naive));

    let days = match window.unwrap_or("7d") {
        "30d" => 30,
        "90d" => 90,
        "365d" => 365,
        other => {
            // `year:YYYY` snaps to the start of that calendar year (a single
            // absolute year, capped one year deep is not applied to explicit
            // year buckets — they are absolute windows).
            if let Some(year_str) = other.strip_prefix("year:")
                && let Ok(year) = year_str.parse::<i32>()
            {
                return Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).single();
            }
            // "7d" and any unrecognised value default to the last 7 days.
            7
        }
    };

    day_start.map(|d| d - Duration::days(days))
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
        ("q" = Option<String>, Query, description = "Combined name search: actor OR target name, case-insensitive substring"),
        ("window" = Option<String>, Query, description = "Relative time window: 7d (default), 30d, 90d, 365d, or year:YYYY"),
        ("since" = Option<String>, Query, description = "Explicit lower time bound (RFC 3339); overrides window"),
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
    let since = resolve_since(query.window.as_deref(), query.since);
    let entries = svc::list_audit_log(
        &state.db,
        query.event_type.as_deref(),
        query.actor,
        query.target_type.as_deref(),
        query.target_id.as_deref(),
        query.target_name.as_deref(),
        query.q.as_deref(),
        since,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Timelike};

    fn day_start_utc() -> DateTime<Utc> {
        let naive = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        Utc.from_utc_datetime(&naive)
    }

    #[test]
    fn resolve_since_defaults_to_seven_days_day_snapped() {
        let since = resolve_since(None, None).expect("default window yields a since");
        // Day-snapped: midnight UTC.
        assert_eq!(since.hour(), 0);
        assert_eq!(since.minute(), 0);
        assert_eq!(since.second(), 0);
        assert_eq!(since, day_start_utc() - Duration::days(7));
    }

    #[test]
    fn resolve_since_maps_known_relative_tiers() {
        assert_eq!(
            resolve_since(Some("30d"), None),
            Some(day_start_utc() - Duration::days(30))
        );
        assert_eq!(
            resolve_since(Some("90d"), None),
            Some(day_start_utc() - Duration::days(90))
        );
        assert_eq!(
            resolve_since(Some("365d"), None),
            Some(day_start_utc() - Duration::days(365))
        );
    }

    #[test]
    fn resolve_since_unrecognised_window_falls_back_to_seven_days() {
        assert_eq!(
            resolve_since(Some("garbage"), None),
            Some(day_start_utc() - Duration::days(7))
        );
    }

    #[test]
    fn resolve_since_year_bucket_snaps_to_year_start() {
        assert_eq!(
            resolve_since(Some("year:2024"), None),
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).single()
        );
    }

    #[test]
    fn resolve_since_explicit_since_overrides_window() {
        let explicit = Utc.with_ymd_and_hms(2023, 6, 1, 12, 0, 0).unwrap();
        assert_eq!(
            resolve_since(Some("90d"), Some(explicit)),
            Some(explicit),
            "explicit since wins over window"
        );
    }
}
