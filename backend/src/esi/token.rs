//! EVE SSO token refresh (OAuth2 `refresh_token` grant).
//!
//! The SSO callback performs the initial `authorization_code` exchange in
//! `handlers/auth.rs`; this is the refresh counterpart, used to obtain a fresh
//! access token from a stored refresh token before an authenticated ESI call.

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

/// A freshly refreshed token set. `refresh_token` may be rotated by EVE SSO, so
/// the caller persists whatever comes back, not the token it sent.
pub struct RefreshedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

/// Exchanges a stored refresh token for a fresh access token via the SSO token
/// endpoint. Returns `None` on any failure (network, non-2xx — e.g. an invalid
/// or revoked refresh token, parse error); a refresh failure is never fatal, it
/// just means "no usable token" to the caller.
pub async fn refresh_access_token(
    http: &reqwest_middleware::ClientWithMiddleware,
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Option<RefreshedTokens> {
    let resp = http
        .post(token_endpoint)
        .basic_auth(client_id, Some(client_secret))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<RefreshResponse>()
        .await
        .ok()?;

    Some(RefreshedTokens {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token,
        access_token_expires_at: Utc::now() + Duration::seconds(resp.expires_in),
    })
}
