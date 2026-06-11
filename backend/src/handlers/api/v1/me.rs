use axum::Json;
use axum::extract::State;

use crate::{
    app_state::AppState,
    dto::account::{AccountDto, CharacterDto, MeDto},
    error::{AppError, ErrorEnvelope},
    handlers::middleware::AuthenticatedAccount,
    response::{ApiResponse, MeResponse},
    services::account as svc,
};

#[utoipa::path(
    get,
    path = "/api/v1/me",
    responses(
        (status = 200, description = "Current account and characters", body = MeResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 429, description = "Rate limited (error.code = \"rate_limited\"); a Retry-After header indicates when to retry. Applies to all /api/* routes via the inbound per-IP limiter.", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "account",
)]
pub async fn get_me(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
) -> Result<Json<ApiResponse<MeDto>>, AppError> {
    let me = svc::get_me(&state.db, account_id).await?;

    let dto = MeDto {
        account: AccountDto::from(me.account),
        characters: me.characters.into_iter().map(CharacterDto::from).collect(),
    };

    Ok(Json(ApiResponse::data(dto)))
}
