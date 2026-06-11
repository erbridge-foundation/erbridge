use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    crypto,
    error::AppError,
    handlers::cookie,
    services::auth::{SsoCompletionInput, SsoOutcome, complete_sso_callback},
    session::{InflightRecord, Session},
};

const ESI_SCOPES: &str = "esi-location.read_location.v1 \
    esi-location.read_ship_type.v1 \
    esi-location.read_online.v1 \
    esi-search.search_structures.v1 \
    esi-ui.write_waypoint.v1";

/// Validates a `return_to` path parameter.
/// Accepts only same-origin paths (starts with exactly one `/`, no `//` or `/\`).
pub(crate) fn validate_return_to(raw: &str) -> Option<String> {
    if !raw.starts_with('/') {
        return None;
    }
    if raw.starts_with("//") || raw.starts_with("/\\") {
        return None;
    }
    if raw.contains('\r') || raw.contains('\n') {
        return None;
    }
    Some(raw.to_string())
}

#[derive(Deserialize)]
pub struct LoginQuery {
    pub return_to: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> Result<Response, AppError> {
    let csrf_state = Uuid::new_v4().to_string();
    let return_to = query.return_to.as_deref().and_then(validate_return_to);

    state
        .inflight_store
        .add(InflightRecord {
            csrf_state: csrf_state.clone(),
            return_to,
            account_id: None,
        })
        .await;

    let redirect_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}/auth/callback&scope={}&state={}",
        state.esi_metadata.authorization_endpoint,
        state.config.esi_client_id,
        state.config.app_url,
        urlencoding::encode(ESI_SCOPES),
        csrf_state,
    );

    Ok(Redirect::to(&redirect_url).into_response())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, AppError> {
    // Find and consume the in-flight OAuth record by csrf_state.
    let inflight = state
        .inflight_store
        .take(&query.state)
        .await
        .ok_or_else(|| AppError::BadRequest("invalid or missing state parameter".to_string()))?;

    if inflight.csrf_state != query.state {
        return Err(AppError::BadRequest("state parameter mismatch".to_string()));
    }

    // Exchange code for tokens.
    let token_resp: TokenResponse = state
        .http_client
        .post(&state.esi_metadata.token_endpoint)
        .basic_auth(
            &state.config.esi_client_id,
            Some(&state.config.esi_client_secret),
        )
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &query.code),
            (
                "redirect_uri",
                &format!("{}/auth/callback", state.config.app_url),
            ),
        ])
        .send()
        .await
        .map_err(|e| AppError::BadGateway(format!("ESI token request failed: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::BadGateway(format!("ESI token endpoint error: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::BadGateway(format!("ESI token parse error: {e}")))?;

    // Parse the access token JWT (no validation against JWKS — ESI tokens are validated
    // structurally only; full JWKS validation is a future hardening step).
    let claims = crate::esi::jwt::parse_claims(&token_resp.access_token)
        .map_err(|e| AppError::BadGateway(format!("invalid ESI access token: {e}")))?;

    let eve_character_id: i64 = claims
        .sub
        .strip_prefix("CHARACTER:EVE:")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::BadGateway("unexpected ESI JWT sub format".to_string()))?;

    let character_name = claims.name;
    let scopes = claims.scp.into_vec();
    let owner_hash = claims.owner;

    // Fetch corporation and alliance IDs from ESI public info.
    let (corporation_id, alliance_id) =
        fetch_character_public_info(&state.http_client, eve_character_id)
            .await
            .map_err(|e| AppError::BadGateway(format!("ESI public info error: {e}")))?;

    // Fetch corp and (optionally) alliance names concurrently.
    use crate::esi::public_info;
    let corp_name_fut = public_info::fetch_corporation_name(&state.http_client, corporation_id);
    let alliance_name = match alliance_id {
        Some(aid) => {
            let (corp_name, alliance_name) = tokio::try_join!(
                corp_name_fut,
                public_info::fetch_alliance_name(&state.http_client, aid)
            )
            .map_err(|e| AppError::BadGateway(format!("ESI public info error: {e}")))?;
            (corp_name, Some(alliance_name))
        }
        None => {
            let corp_name = corp_name_fut
                .await
                .map_err(|e| AppError::BadGateway(format!("ESI public info error: {e}")))?;
            (corp_name, None)
        }
    };
    let (corporation_name, alliance_name) = alliance_name;

    let encryption_key = crypto::token_encryption_key(&state.config.encryption_secret)?;
    let access_token_expires_at = Utc::now() + Duration::seconds(token_resp.expires_in);

    let outcome = complete_sso_callback(
        &state.db,
        SsoCompletionInput {
            add_character_account_id: inflight.account_id,
            eve_character_id,
            character_name: &character_name,
            corporation_id,
            corporation_name: &corporation_name,
            alliance_id,
            alliance_name: alliance_name.as_deref(),
            esi_client_id: &state.config.esi_client_id,
            access_token: &token_resp.access_token,
            refresh_token: &token_resp.refresh_token,
            access_token_expires_at,
            scopes: &scopes,
            owner_hash: &owner_hash,
            encryption_key: &encryption_key,
        },
    )
    .await?;

    // A blocked character gets no session and no cookie — just an informational
    // redirect. This covers both the login and add-character flows.
    let account_id = match outcome {
        SsoOutcome::Authenticated(id) => id,
        SsoOutcome::Blocked => return Ok(Redirect::to("/blocked").into_response()),
    };

    // Create the persistent session row. The session ID is a fresh UUID; the
    // cookie carries it as a signed JWT.
    let session_id = Uuid::new_v4().to_string();
    state
        .session_store
        .add(&session_id, account_id, None, inflight.account_id.is_some())
        .await
        .map_err(AppError::Internal)?;

    let jwt_key = crypto::jwt_signing_key(&state.config.encryption_secret)?;
    let jwt = crypto::sign_session_jwt(&session_id, &jwt_key)?;

    let redirect_path = inflight.return_to.as_deref().unwrap_or("/");
    let mut response = Redirect::to(redirect_path).into_response();
    cookie::set_session_cookie(response.headers_mut(), &jwt);

    Ok(response)
}

async fn fetch_character_public_info(
    client: &reqwest_middleware::ClientWithMiddleware,
    eve_character_id: i64,
) -> anyhow::Result<(i64, Option<i64>)> {
    #[derive(Deserialize)]
    struct PublicInfo {
        corporation_id: i64,
        alliance_id: Option<i64>,
    }

    let url = format!("https://esi.evetech.net/latest/characters/{eve_character_id}/");
    let info: PublicInfo = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .error_for_status()?
        .json()
        .await?;

    Ok((info.corporation_id, info.alliance_id))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let session_id = cookie::extract_session_jwt(&headers).and_then(|jwt| {
        let key = crypto::jwt_signing_key(&state.config.encryption_secret).ok()?;
        crypto::verify_session_jwt(&jwt, &key).ok()
    });

    if let Some(sid) = session_id {
        // Best-effort delete; ignore DB errors so logout always clears the cookie.
        let _ = state.session_store.remove(&sid).await;
    }

    let mut response = Redirect::to("/").into_response();
    cookie::clear_session_cookie(response.headers_mut());
    response
}

#[derive(Deserialize)]
pub struct AddCharacterQuery {
    pub return_to: Option<String>,
}

pub async fn add_character(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AddCharacterQuery>,
) -> Result<Response, AppError> {
    // Require existing session.
    let session = extract_session(&state, &headers)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let csrf_state = Uuid::new_v4().to_string();
    let return_to = query.return_to.as_deref().and_then(validate_return_to);

    state
        .inflight_store
        .add(InflightRecord {
            csrf_state: csrf_state.clone(),
            return_to,
            account_id: Some(session.account_id),
        })
        .await;

    let redirect_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}/auth/callback&scope={}&state={}",
        state.esi_metadata.authorization_endpoint,
        state.config.esi_client_id,
        state.config.app_url,
        urlencoding::encode(ESI_SCOPES),
        csrf_state,
    );

    Ok(Redirect::to(&redirect_url).into_response())
}

pub async fn extract_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<Session>, AppError> {
    let Some(jwt) = cookie::extract_session_jwt(headers) else {
        return Ok(None);
    };
    let key =
        crypto::jwt_signing_key(&state.config.encryption_secret).map_err(AppError::Internal)?;
    let Ok(session_id) = crypto::verify_session_jwt(&jwt, &key) else {
        return Ok(None);
    };
    state
        .session_store
        .get(&session_id)
        .await
        .map_err(AppError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_return_to_accepts_valid_paths() {
        assert_eq!(
            validate_return_to("/characters"),
            Some("/characters".to_string())
        );
        assert_eq!(validate_return_to("/"), Some("/".to_string()));
        assert_eq!(
            validate_return_to("/maps?foo=bar"),
            Some("/maps?foo=bar".to_string())
        );
    }

    #[test]
    fn validate_return_to_rejects_absolute_url() {
        assert_eq!(validate_return_to("https://evil.example.com/"), None);
        assert_eq!(validate_return_to("http://evil.com"), None);
    }

    #[test]
    fn validate_return_to_rejects_scheme_relative() {
        assert_eq!(validate_return_to("//evil.example.com/"), None);
    }

    #[test]
    fn validate_return_to_rejects_backslash_scheme_relative() {
        assert_eq!(validate_return_to("/\\evil.com"), None);
    }

    #[test]
    fn validate_return_to_rejects_newlines() {
        assert_eq!(validate_return_to("/path\r\ninjected"), None);
        assert_eq!(validate_return_to("/path\ninjected"), None);
    }

    #[test]
    fn validate_return_to_rejects_empty() {
        assert_eq!(validate_return_to(""), None);
    }
}
