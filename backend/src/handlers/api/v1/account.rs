use axum::{extract::State, http::StatusCode};

use crate::{
    app_state::AppState,
    error::{AppError, ErrorEnvelope},
    handlers::{cookie, middleware::AuthenticatedAccount},
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
) -> Result<(StatusCode, axum::http::HeaderMap), AppError> {
    account_service::delete_account(&state.db, account_id).await?;

    state
        .session_store
        .remove_all_for_account(account_id)
        .await
        .map_err(AppError::Internal)?;

    let mut headers = axum::http::HeaderMap::new();
    cookie::clear_session_cookie(&mut headers);

    Ok((StatusCode::NO_CONTENT, headers))
}
