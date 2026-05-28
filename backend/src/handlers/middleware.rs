use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::{FromRef, FromRequestParts},
    http::{Request, request::Parts},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    db::accounts,
    error::AppError,
    handlers::{cookie, crypto},
    services::api_keys as svc,
};

/// A request-scoped slot the middleware places into `request.extensions` so
/// the `AuthenticatedAccount` extractor (which sees `Parts`, not the response)
/// can communicate a freshly-minted session JWT back to the wrapping layer.
///
/// API-key requests never write to this slot, so API-key callers never get a
/// refreshed cookie.
///
/// Handlers that intentionally log the user out (e.g. `DELETE /api/v1/account`)
/// MUST call `suppress()` so the middleware does not overwrite the handler's
/// cookie-clearing `Set-Cookie` header with a refreshed session cookie.
#[derive(Clone, Default)]
pub struct RefreshedJwtSlot(Arc<Mutex<Option<String>>>);

impl RefreshedJwtSlot {
    /// Empty the slot so the wrapping middleware writes no refreshed
    /// `Set-Cookie` header. Use this in handlers that end the session.
    pub fn suppress(&self) {
        #[allow(clippy::unwrap_used)]
        {
            *self.0.lock().unwrap() = None;
        }
    }
}

/// Axum extractor that resolves the authenticated account ID from either:
/// 1. `Authorization: Bearer erb_…` (API key, account-scoped only)
/// 2. Session cookie (falls back when no bearer header is present or prefix doesn't match)
///
/// Rejects soft-deleted accounts with 401 `account_soft_deleted` when using an API key.
///
/// On successful cookie-based auth, the extractor writes a freshly-minted
/// session JWT into the request-scoped `RefreshedJwtSlot`; the
/// `refresh_session_cookie` middleware reads it on the way out and writes a
/// `Set-Cookie` header.
pub struct AuthenticatedAccount(pub Uuid);

impl<S> FromRequestParts<S> for AuthenticatedAccount
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        // 1. Try Bearer token.
        if let Some(bearer_value) = extract_bearer(&parts.headers)
            && bearer_value.starts_with(crate::handlers::api_key::PREFIX)
        {
            let row = svc::lookup_by_plaintext(&state.db, &bearer_value).await?;

            return match row {
                Some(r) if r.scope == "account" => {
                    let account_id = r.account_id.ok_or(AppError::Unauthorized)?;
                    // Reject if the account has been soft-deleted.
                    let account = accounts::get_account(&state.db, account_id)
                        .await
                        .map_err(AppError::Internal)?;
                    match account {
                        Some(a) if a.status == "soft_deleted" => Err(AppError::AccountSoftDeleted),
                        Some(_) => Ok(AuthenticatedAccount(account_id)),
                        None => Err(AppError::Unauthorized),
                    }
                }
                Some(_) => Err(AppError::Forbidden),
                None => Err(AppError::Unauthorized),
            };
        }
        // Bearer present but not erb_ prefix — falls through to session below.

        // 2. Try session cookie.
        let jwt = cookie::extract_session_jwt(&parts.headers).ok_or(AppError::Unauthorized)?;
        let key_bytes =
            crypto::jwt_signing_key(&state.config.encryption_secret).map_err(AppError::Internal)?;
        let session_id =
            crypto::verify_session_jwt(&jwt, &key_bytes).map_err(|_| AppError::Unauthorized)?;
        let session = state
            .session_store
            .get(&session_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or(AppError::Unauthorized)?;

        // Mint a fresh session JWT (exp = now + 7d) so the cookie's lifetime
        // tracks the row's `expires_at`. Write it into the request-scoped
        // slot installed by `refresh_session_cookie`. If the slot is absent
        // (e.g. unit test with no wrapping middleware) the refresh is
        // silently a no-op — the auth itself still succeeds.
        let fresh_jwt =
            crypto::sign_session_jwt(&session_id, &key_bytes).map_err(AppError::Internal)?;
        if let Some(slot) = parts.extensions.get::<RefreshedJwtSlot>() {
            #[allow(clippy::unwrap_used)]
            {
                *slot.0.lock().unwrap() = Some(fresh_jwt);
            }
        }

        Ok(AuthenticatedAccount(session.account_id))
    }
}

/// Axum middleware that installs a per-request `RefreshedJwtSlot`, runs the
/// inner stack, and (if the slot was filled by the extractor) writes a fresh
/// `Set-Cookie` header back on the response.
///
/// API-key requests never write to the slot, so they never receive a refreshed
/// cookie.
pub async fn refresh_session_cookie(mut req: Request<Body>, next: Next) -> Response {
    let slot = RefreshedJwtSlot::default();
    req.extensions_mut().insert(slot.clone());

    let mut response = next.run(req).await;

    let jwt = {
        #[allow(clippy::unwrap_used)]
        let guard = slot.0.lock().unwrap();
        guard.clone()
    };
    if let Some(jwt) = jwt {
        cookie::set_session_cookie(response.headers_mut(), &jwt);
    }
    response
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
    fn refreshed_jwt_slot_suppress_clears_value() {
        let slot = RefreshedJwtSlot::default();
        #[allow(clippy::unwrap_used)]
        {
            *slot.0.lock().unwrap() = Some("would-be-refresh-jwt".to_string());
        }
        slot.suppress();
        #[allow(clippy::unwrap_used)]
        let guard = slot.0.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn refreshed_jwt_slot_suppress_is_idempotent_on_empty_slot() {
        let slot = RefreshedJwtSlot::default();
        slot.suppress();
        slot.suppress();
        #[allow(clippy::unwrap_used)]
        let guard = slot.0.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn extract_bearer_with_non_erb_prefix_returns_value() {
        // The function extracts the value; the caller checks the prefix.
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer some_other_token"),
        );
        assert_eq!(
            extract_bearer(&headers),
            Some("some_other_token".to_string())
        );
    }
}
