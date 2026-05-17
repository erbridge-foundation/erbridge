use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use uuid::Uuid;

use crate::{app_state::AppState, db::accounts, error::AppError, handlers::{cookie, crypto}, services::api_keys as svc};

/// Axum extractor that resolves the authenticated account ID from either:
/// 1. `Authorization: Bearer erb_…` (API key, account-scoped only)
/// 2. Session cookie (falls back when no bearer header is present or prefix doesn't match)
///
/// Rejects soft-deleted accounts with 401 `account_soft_deleted` when using an API key.
pub struct AuthenticatedAccount(pub Uuid);

impl<S> FromRequestParts<S> for AuthenticatedAccount
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        // 1. Try Bearer token.
        if let Some(bearer_value) = extract_bearer(&parts.headers) {
            if bearer_value.starts_with(crate::handlers::api_key::PREFIX) {
                let row = svc::lookup_by_plaintext(&state.db, &bearer_value).await?;

                return match row {
                    Some(r) if r.scope == "account" => {
                        let account_id = r.account_id.ok_or(AppError::Unauthorized)?;
                        // Reject if the account has been soft-deleted.
                        let account = accounts::get_account(&state.db, account_id)
                            .await
                            .map_err(AppError::Internal)?;
                        match account {
                            Some(a) if a.status == "soft_deleted" => {
                                Err(AppError::AccountSoftDeleted)
                            }
                            Some(_) => Ok(AuthenticatedAccount(account_id)),
                            None => Err(AppError::Unauthorized),
                        }
                    }
                    Some(_) => Err(AppError::Forbidden),
                    None => Err(AppError::Unauthorized),
                };
            }
            // Bearer present but not erb_ prefix — fall through to session.
        }

        // 2. Try session cookie.
        let jwt = cookie::extract_session_jwt(&parts.headers).ok_or(AppError::Unauthorized)?;
        let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret)
            .map_err(AppError::Internal)?;
        let session_id = crypto::verify_session_jwt(&jwt, &key_bytes)
            .map_err(|_| AppError::Unauthorized)?;
        let session = state
            .session_store
            .get(&session_id)
            .await
            .ok_or(AppError::Unauthorized)?;

        Ok(AuthenticatedAccount(session.account_id))
    }
}

fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<String> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    auth.strip_prefix("Bearer ").map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn extract_bearer_finds_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer erb_abc123"),
        );
        assert_eq!(extract_bearer(&headers), Some("erb_abc123".to_string()));
    }

    #[test]
    fn extract_bearer_returns_none_when_absent() {
        let headers = HeaderMap::new();
        assert!(extract_bearer(&headers).is_none());
    }

    #[test]
    fn extract_bearer_returns_none_for_non_bearer_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );
        assert!(extract_bearer(&headers).is_none());
    }

    #[test]
    fn extract_bearer_with_non_erb_prefix_returns_value() {
        // The function extracts the value; the caller checks the prefix.
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer some_other_token"),
        );
        assert_eq!(extract_bearer(&headers), Some("some_other_token".to_string()));
    }
}
