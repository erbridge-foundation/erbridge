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
    handlers::{cookie, crypto},
    db::{accounts, characters},
    error::AppError,
    session::Session,
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

    // Create an in-flight session with no account yet — we use a placeholder
    // account ID; the callback will resolve and replace with the real account.
    // We need a session entry to store csrf_state and return_to.
    let session = Session {
        session_id: format!("inflight_{csrf_state}"),
        account_id: Uuid::nil(),
        csrf_state: Some(csrf_state.clone()),
        return_to,
        add_character_mode: false,
    };
    state.session_store.add(session).await;

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

#[derive(Deserialize)]
struct EsiJwtClaims {
    sub: String,
    name: String,
}

pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, AppError> {
    // Find in-flight session by state value.
    let inflight_id = format!("inflight_{}", query.state);
    let inflight = state
        .session_store
        .get(&inflight_id)
        .await
        .ok_or_else(|| AppError::BadRequest("invalid or missing state parameter".to_string()))?;

    if inflight.csrf_state.as_deref() != Some(query.state.as_str()) {
        return Err(AppError::BadRequest(
            "state parameter mismatch".to_string(),
        ));
    }

    state.session_store.remove(&inflight_id).await;

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
            ("redirect_uri", &format!("{}/auth/callback", state.config.app_url)),
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
    let claims = parse_esi_jwt_claims(&token_resp.access_token)
        .map_err(|e| AppError::BadGateway(format!("invalid ESI access token: {e}")))?;

    let eve_character_id: i64 = claims
        .sub
        .strip_prefix("CHARACTER:EVE:")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::BadGateway("unexpected ESI JWT sub format".to_string()))?;

    let character_name = claims.name;

    // Fetch corporation and alliance IDs from ESI public info.
    let (corporation_id, alliance_id) =
        fetch_character_public_info(&state.http_client, eve_character_id)
            .await
            .map_err(|e| AppError::BadGateway(format!("ESI public info error: {e}")))?;

    let encryption_key = crypto::token_encryption_key(&state.config.encryption_secret)
        .map_err(anyhow::Error::from)?;
    let expires_at = Utc::now() + Duration::seconds(token_resp.expires_in);

    // Single Postgres transaction composing the DB steps.
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(anyhow::Error::from)?;

    let add_character_account_id = if inflight.add_character_mode {
        Some(inflight.account_id)
    } else {
        None
    };

    let account_id =
        accounts::resolve_or_create(&mut tx, add_character_account_id, eve_character_id)
            .await?;

    accounts::reactivate_if_soft_deleted(&mut tx, account_id).await?;

    let character_id = characters::upsert_tokens(
        &mut tx,
        account_id,
        eve_character_id,
        &character_name,
        corporation_id,
        alliance_id,
        &state.config.esi_client_id,
        &token_resp.access_token,
        &token_resp.refresh_token,
        expires_at,
        &encryption_key,
    )
    .await?;

    characters::promote_if_no_main(&mut tx, account_id, character_id).await?;

    tx.commit().await.map_err(anyhow::Error::from)?;

    // Create persistent session.
    let session_id = Uuid::new_v4().to_string();
    let new_session = Session {
        session_id: session_id.clone(),
        account_id,
        csrf_state: None,
        return_to: None,
        add_character_mode: false,
    };
    state.session_store.add(new_session).await;

    let jwt_key = crypto::jwt_signing_key(&state.config.encryption_secret)
        .map_err(anyhow::Error::from)?;
    let jwt = crypto::sign_session_jwt(&session_id, &jwt_key).map_err(anyhow::Error::from)?;

    let redirect_path = inflight.return_to.as_deref().unwrap_or("/");
    let mut response = Redirect::to(redirect_path).into_response();
    cookie::set_session_cookie(response.headers_mut(), &jwt);

    Ok(response)
}

fn parse_esi_jwt_claims(token: &str) -> anyhow::Result<EsiJwtClaims> {
    // Decode without verification (ESI tokens are verified by signature against jwks_uri;
    // full JWKS validation is a future hardening step — we trust ESI's endpoint).
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("malformed JWT"));
    }
    let payload = parts[1];
    // base64url decode with padding
    let padded = match payload.len() % 4 {
        0 => payload.to_string(),
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => return Err(anyhow::anyhow!("invalid base64url padding")),
    };
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| {
            use base64::{engine::general_purpose::URL_SAFE, Engine};
            URL_SAFE.decode(&padded)
        })
        .map_err(|e| anyhow::anyhow!("base64 decode error: {e}"))?;
    serde_json::from_slice(&decoded).map_err(|e| anyhow::anyhow!("JWT claims parse error: {e}"))
}

async fn fetch_character_public_info(
    client: &reqwest::Client,
    eve_character_id: i64,
) -> anyhow::Result<(i64, Option<i64>)> {
    #[derive(Deserialize)]
    struct PublicInfo {
        corporation_id: i64,
        alliance_id: Option<i64>,
    }

    let url = format!(
        "https://esi.evetech.net/latest/characters/{eve_character_id}/"
    );
    let info: PublicInfo = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok((info.corporation_id, info.alliance_id))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let session_id = cookie::extract_session_jwt(&headers)
        .and_then(|jwt| {
            let key = crypto::jwt_signing_key(&state.config.encryption_secret).ok()?;
            crypto::verify_session_jwt(&jwt, &key).ok()
        });

    if let Some(sid) = session_id {
        state.session_store.remove(&sid).await;
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
        .await
        .ok_or(AppError::Unauthorized)?;

    let csrf_state = Uuid::new_v4().to_string();
    let return_to = query.return_to.as_deref().and_then(validate_return_to);

    // Replace session with add_character_mode = true.
    let updated = Session {
        session_id: session.session_id.clone(),
        account_id: session.account_id,
        csrf_state: Some(csrf_state.clone()),
        return_to,
        add_character_mode: true,
    };
    // Store an in-flight record keyed by csrf_state, preserving account_id.
    let inflight = Session {
        session_id: format!("inflight_{csrf_state}"),
        account_id: session.account_id,
        csrf_state: Some(csrf_state.clone()),
        return_to: updated.return_to.clone(),
        add_character_mode: true,
    };
    state.session_store.add(inflight).await;

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

pub async fn extract_session(state: &AppState, headers: &HeaderMap) -> Option<Session> {
    let jwt = cookie::extract_session_jwt(headers)?;
    let key = crypto::jwt_signing_key(&state.config.encryption_secret).ok()?;
    let session_id = crypto::verify_session_jwt(&jwt, &key).ok()?;
    state.session_store.get(&session_id).await
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
