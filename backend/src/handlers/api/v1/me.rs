use axum::extract::State;
use axum::Json;

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
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "account",
)]
pub async fn get_me(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
) -> Result<Json<ApiResponse<MeDto>>, AppError> {
    let me = svc::get_me(&state.db, &state.http_client, account_id).await?;

    let dto = MeDto {
        account: AccountDto::from(me.account),
        characters: me.characters.into_iter().map(CharacterDto::from).collect(),
    };

    Ok(Json(ApiResponse::data(dto)))
}
