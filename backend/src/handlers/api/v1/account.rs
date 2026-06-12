use axum::{Extension, extract::State, http::StatusCode};

use crate::{
    app_state::AppState,
    error::{AppError, ErrorEnvelope},
    handlers::{
        cookie,
        middleware::{AuthenticatedAccount, RefreshedJwtSlot},
    },
    services::account as account_service,
};

#[utoipa::path(
    delete,
    path = "/api/v1/account",
    responses(
        (status = 204, description = "Account soft-deleted and session cleared"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
        (status = 409, description = "Cannot remove the last server admin", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "account",
)]
pub async fn delete_account(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    Extension(refresh_slot): Extension<RefreshedJwtSlot>,
) -> Result<(StatusCode, axum::http::HeaderMap), AppError> {
    // The service soft-deletes the account and deletes its sessions in one
    // transaction; the handler only owns the response-side cookie concerns.
    account_service::delete_account(&state.db, account_id).await?;

    // Stop the wrapping `refresh_session_cookie` middleware from overwriting
    // the cleared cookie with a freshly-minted session JWT.
    refresh_slot.suppress();

    let mut headers = axum::http::HeaderMap::new();
    cookie::clear_session_cookie(&mut headers);

    Ok((StatusCode::NO_CONTENT, headers))
}
