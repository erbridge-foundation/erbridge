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
    crypto,
    db::{accounts, blocks},
    error::AppError,
    handlers::cookie,
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
            && bearer_value.starts_with(crate::api_key::PREFIX)
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
                        Some(_) => {
                            // Bearer is the one auth route that survives block
                            // teardown (API keys are not deleted on block), so
                            // it carries the explicit block check. The session
                            // cookie path needs none — block deletes the
                            // sessions, identical to soft-delete enforcement.
                            if blocks::account_has_blocked_character(&state.db, account_id)
                                .await
                                .map_err(AppError::Internal)?
                            {
                                return Err(AppError::AccountBlocked);
                            }
                            Ok(AuthenticatedAccount(account_id))
                        }
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

/// Axum extractor that resolves an authenticated **server-admin** account ID.
///
/// Unlike [`AuthenticatedAccount`], this extractor is **session-cookie only**:
/// it deliberately does NOT consult `Authorization: Bearer erb_…` API keys. A
/// leaked API key must never confer admin power — an admin action requires a
/// fresh-ish (7-day sliding) browser session. (The first-class story for server
/// automation remains the `scope = 'server'` API key, not admin user-actions.)
///
/// Rejections:
/// - no/invalid session cookie → `AppError::Unauthorized` (401 `unauthenticated`)
/// - valid session for a non-admin account → `AppError::ForbiddenAdminRequired`
///   (403 `forbidden_admin_required`)
///
/// Unlike the cookie path of `AuthenticatedAccount`, this extractor does NOT
/// refresh the session cookie. Admin actions are infrequent and always
/// accompanied by ordinary `/me`-style requests that already slide the window;
/// keeping the refresh out of here keeps the extractor's single responsibility
/// (authorise an admin) clean.
#[derive(Debug)]
pub struct AdminAccount(pub Uuid);

impl<S> FromRequestParts<S> for AdminAccount
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        // Session cookie only — never the bearer header.
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

        // Load the account and require the admin flag. A missing account row for
        // a live session is an invariant violation, treated as unauthenticated.
        let account = accounts::get_account(&state.db, session.account_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or(AppError::Unauthorized)?;

        if !account.is_server_admin {
            return Err(AppError::ForbiddenAdminRequired);
        }

        Ok(AdminAccount(session.account_id))
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

    // ── AdminAccount extractor ────────────────────────────────────────────────

    use crate::{
        app_state::AppState,
        config::Config,
        crypto,
        db::accounts,
        esi::EsiMetadata,
        session::{InflightStore, SessionStore},
    };
    use axum::http::{Request, header};
    use sqlx::PgPool;
    use std::sync::Arc;

    const TEST_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn build_state(pool: PgPool) -> AppState {
        AppState {
            config: Arc::new(Config {
                app_url: "http://localhost:3000".into(),
                encryption_secret: TEST_SECRET.into(),
                esi_client_id: "test_client_id".into(),
                esi_client_secret: "test_client_secret".into(),
                database_url: String::new(),
                rate_limit: Default::default(),
            }),
            db: pool.clone(),
            esi_metadata: Arc::new(EsiMetadata {
                authorization_endpoint: "https://login.eveonline.com/v2/oauth/authorize".into(),
                token_endpoint: "https://login.eveonline.com/v2/oauth/token".into(),
                jwks_uri: "https://login.eveonline.com/oauth/jwks".into(),
            }),
            session_store: SessionStore::new(pool.clone()),
            inflight_store: InflightStore::new(),
            http_client: reqwest::Client::new().into(),
        }
    }

    /// Creates an account (optionally promoted to admin) with a live session and
    /// returns `(account_id, session_cookie_value)`.
    async fn account_with_session(state: &AppState, admin: bool) -> (Uuid, String) {
        let account_id = accounts::create_account(&state.db).await.unwrap();
        if admin {
            let mut tx = state.db.begin().await.unwrap();
            accounts::set_server_admin(&mut tx, account_id, true)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        let session_id = Uuid::new_v4().to_string();
        state
            .session_store
            .add(&session_id, account_id, None, false)
            .await
            .unwrap();
        let key_bytes = crypto::jwt_signing_key(&state.config.encryption_secret).unwrap();
        let jwt = crypto::sign_session_jwt(&session_id, &key_bytes).unwrap();
        (account_id, format!("session={jwt}"))
    }

    /// Drives the `AdminAccount` extractor against a constructed request.
    async fn extract_admin(
        state: &AppState,
        cookie: Option<&str>,
        bearer: Option<&str>,
    ) -> Result<AdminAccount, AppError> {
        let mut builder = Request::builder().uri("/api/v1/admin/accounts");
        if let Some(c) = cookie {
            builder = builder.header(header::COOKIE, c);
        }
        if let Some(b) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {b}"));
        }
        let req = builder.body(Body::empty()).unwrap();
        let (mut parts, _) = req.into_parts();
        AdminAccount::from_request_parts(&mut parts, state).await
    }

    #[sqlx::test]
    async fn admin_extractor_rejects_no_cookie_with_401(pool: PgPool) {
        let state = build_state(pool);
        let err = extract_admin(&state, None, None).await.unwrap_err();
        assert!(matches!(err, AppError::Unauthorized));
    }

    #[sqlx::test]
    async fn admin_extractor_rejects_non_admin_with_403(pool: PgPool) {
        let state = build_state(pool);
        let (_id, cookie) = account_with_session(&state, false).await;
        let err = extract_admin(&state, Some(&cookie), None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::ForbiddenAdminRequired));
    }

    #[sqlx::test]
    async fn admin_extractor_accepts_admin_cookie(pool: PgPool) {
        let state = build_state(pool);
        let (account_id, cookie) = account_with_session(&state, true).await;
        let AdminAccount(resolved) = extract_admin(&state, Some(&cookie), None).await.unwrap();
        assert_eq!(resolved, account_id);
    }

    #[sqlx::test]
    async fn admin_extractor_ignores_bearer_key_for_admin_account(pool: PgPool) {
        // An account-scoped API key whose account IS an admin must NOT confer
        // admin via the bearer header — the extractor is cookie-only, so with no
        // cookie present it rejects with 401 regardless of the key.
        let state = build_state(pool);
        let account_id = accounts::create_account(&state.db).await.unwrap();
        let mut tx = state.db.begin().await.unwrap();
        accounts::set_server_admin(&mut tx, account_id, true)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let key = crate::services::api_keys::create_key(&state.db, account_id, "k", None)
            .await
            .unwrap();

        let err = extract_admin(&state, None, Some(&key.plaintext))
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::Unauthorized),
            "bearer key must never satisfy the cookie-only admin extractor"
        );
    }
}
