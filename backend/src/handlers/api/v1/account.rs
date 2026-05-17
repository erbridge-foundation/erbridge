use axum::{extract::State, http::StatusCode};

use crate::{
    app_state::AppState,
    db::accounts,
    error::{AppError, ErrorEnvelope},
    handlers::{cookie, middleware::AuthenticatedAccount},
};

#[utoipa::path(
    delete,
    path = "/api/v1/account",
    responses(
        (status = 204, description = "Account soft-deleted and session cleared"),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "account",
)]
pub async fn delete_account(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
) -> Result<(StatusCode, axum::http::HeaderMap), AppError> {
    accounts::soft_delete(&state.db, account_id)
        .await
        .map_err(AppError::Internal)?;

    state.session_store.remove_all_for_account(account_id).await;

    let mut headers = axum::http::HeaderMap::new();
    cookie::clear_session_cookie(&mut headers);

    Ok((StatusCode::NO_CONTENT, headers))
}
