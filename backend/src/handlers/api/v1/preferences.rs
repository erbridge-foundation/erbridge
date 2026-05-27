use axum::Json;
use axum::extract::State;

use crate::{
    app_state::AppState,
    dto::preferences::{PreferencesDto, PreferencesPatch},
    error::{AppError, ErrorEnvelope},
    handlers::middleware::AuthenticatedAccount,
    response::{ApiResponse, PreferencesResponse},
    services::preferences as svc,
};

#[utoipa::path(
    get,
    path = "/api/v1/me/preferences",
    responses(
        (status = 200, description = "The account's accessibility preferences", body = PreferencesResponse),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "preferences",
)]
pub async fn get_preferences(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
) -> Result<Json<ApiResponse<PreferencesDto>>, AppError> {
    let prefs = svc::get_preferences(&state.db, account_id).await?;
    Ok(Json(ApiResponse::data(prefs)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/me/preferences",
    request_body = PreferencesPatch,
    responses(
        (status = 200, description = "The full merged preference set", body = PreferencesResponse),
        (status = 400, description = "Unknown key or invalid value", body = ErrorEnvelope),
        (status = 401, description = "Unauthenticated", body = ErrorEnvelope),
    ),
    security(("session_cookie" = []), ("bearer_token" = [])),
    tag = "preferences",
)]
pub async fn update_preferences(
    State(state): State<AppState>,
    AuthenticatedAccount(account_id): AuthenticatedAccount,
    body: Json<serde_json::Value>,
) -> Result<Json<ApiResponse<PreferencesDto>>, AppError> {
    // Deserialise in the handler so an unknown key or invalid enum value maps
    // to 400 (the documented contract), not axum's default 422 JsonRejection.
    let patch: PreferencesPatch = serde_json::from_value(body.0)
        .map_err(|e| AppError::BadRequest(format!("invalid preferences patch: {e}")))?;

    let prefs = svc::update_preferences(&state.db, account_id, patch).await?;
    Ok(Json(ApiResponse::data(prefs)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_body_with_unknown_key_is_bad_request() {
        let body = Json(serde_json::json!({"not_a_pref": "x"}));
        let patch: Result<PreferencesPatch, _> = serde_json::from_value(body.0);
        // Mirrors the handler's mapping: a deserialise error becomes BadRequest.
        let mapped = patch.map_err(|e| AppError::BadRequest(e.to_string()));
        assert!(matches!(mapped, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn patch_body_with_valid_keys_deserialises() {
        let body = Json(serde_json::json!({"text_size": "large", "locale": "en"}));
        let patch: PreferencesPatch = serde_json::from_value(body.0).unwrap();
        assert!(!patch.is_empty());
        assert_eq!(patch.locale, Some(crate::dto::preferences::Locale::En));
    }
}
