//! EVE SSO access-token JWT verification and claim parsing.
//!
//! Both the SSO callback (`handlers/auth.rs`) and the token-refresh path
//! (`esi/token.rs`) receive an access token that is a JWT carrying the
//! character identity. This module owns verifying that JWT and decoding its
//! claims so the call sites share one funnel.
//!
//! Verification is mandatory: the token's signature is checked against the SSO
//! JWKS (see [`super::jwks`]) for the key matching the token's `kid`, and its
//! `exp` expiry and `iss` issuer are validated, before any claim is trusted.
//! There is deliberately no unverified parser — a future call site cannot
//! quietly skip verification.

use anyhow::{Context, Result, anyhow};
use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use serde::Deserialize;

use super::jwks::JwksCache;

/// The expected `iss` claim on EVE SSO access tokens.
pub const EVE_ISSUER: &str = "https://login.eveonline.com";

/// Claims we read from an EVE SSO access-token JWT.
#[derive(Debug, Deserialize)]
pub struct EsiJwtClaims {
    /// `CHARACTER:EVE:<character-id>`.
    pub sub: String,
    /// Character name.
    pub name: String,
    /// Granted scopes — a single string or an array (see [`Scp`]).
    #[serde(default)]
    pub scp: Scp,
    /// The character owner hash. CCP rotates this when a character is
    /// transferred to a different account; it is the canonical transfer signal.
    pub owner: String,
}

/// EVE's `scp` claim is a single string when one scope is granted, or an array
/// when multiple scopes are granted.
#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
pub enum Scp {
    #[default]
    None,
    One(String),
    Many(Vec<String>),
}

impl Scp {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            Scp::None => vec![],
            Scp::One(s) => vec![s],
            Scp::Many(v) => v,
        }
    }
}

/// Verifies an EVE SSO access-token JWT against the SSO JWKS and returns its
/// claims. Checks the RS256 signature (against the key matching the token's
/// `kid`, refetching the JWKS on an unknown `kid`), the `exp` expiry (with
/// jsonwebtoken's default 60s leeway), and the `iss` issuer. Returns an error
/// for a malformed token, an unknown signing key, a bad signature, an expired
/// token, a wrong issuer, or claims missing a required field (notably `owner`).
pub async fn verify_and_parse(token: &str, jwks: &JwksCache) -> Result<EsiJwtClaims> {
    let header = decode_header(token).context("malformed JWT header")?;
    let kid = header.kid.ok_or_else(|| anyhow!("JWT header has no kid"))?;
    let key = jwks.key_for(&kid).await?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[EVE_ISSUER]);
    // EVE's `aud` shape is validated as a follow-up (see design.md); issuer +
    // signature + expiry close the gap this change targets.
    validation.validate_aud = false;

    let data = decode::<EsiJwtClaims>(token, &key, &validation)
        .context("ESI access-token JWT failed verification")?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::esi::jwks::JwksCache;
    use crate::esi::test_support::{EsiClaims, TestKeypair, jwks_json, test_keypair};
    use reqwest_middleware::ClientBuilder;

    fn http() -> reqwest_middleware::ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new()).build()
    }

    /// A cache holding exactly the public key for `kp`, no network refetch.
    fn cache_for(kp: &TestKeypair) -> JwksCache {
        let body = jwks_json(&[kp]);
        let keys = super::super::jwks::decode_keys_for_test(body.as_bytes());
        JwksCache::from_keys_for_test(http(), "http://unused", keys)
    }

    #[tokio::test]
    async fn verifies_and_parses_a_valid_token() {
        let kp = test_keypair("kid-1");
        let token = kp.sign(&EsiClaims::valid(123, "Pilot", "hash-abc"));
        let claims = verify_and_parse(&token, &cache_for(&kp)).await.unwrap();
        assert_eq!(claims.sub, "CHARACTER:EVE:123");
        assert_eq!(claims.name, "Pilot");
        assert_eq!(claims.owner, "hash-abc");
        assert_eq!(claims.scp.into_vec(), vec!["publicData".to_string()]);
    }

    #[tokio::test]
    async fn rejects_token_signed_by_an_unknown_key() {
        // Signed by one key, verified against a cache holding a different key
        // (and an unreachable refetch URI), so verification cannot succeed.
        let signer = test_keypair("kid-signer");
        let other = test_keypair("kid-other");
        let token = signer.sign(&EsiClaims::valid(1, "P", "h"));
        let cache = JwksCache::from_keys_for_test(
            http(),
            "http://127.0.0.1:0/unreachable",
            super::super::jwks::decode_keys_for_test(jwks_json(&[&other]).as_bytes()),
        );
        assert!(verify_and_parse(&token, &cache).await.is_err());
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let kp = test_keypair("kid-1");
        let mut claims = EsiClaims::valid(1, "P", "h");
        // Past the 60s default leeway.
        claims.exp = chrono::Utc::now().timestamp() - 120;
        let token = kp.sign(&claims);
        assert!(verify_and_parse(&token, &cache_for(&kp)).await.is_err());
    }

    #[tokio::test]
    async fn rejects_wrong_issuer() {
        let kp = test_keypair("kid-1");
        let mut claims = EsiClaims::valid(1, "P", "h");
        claims.iss = "https://evil.example".to_string();
        let token = kp.sign(&claims);
        assert!(verify_and_parse(&token, &cache_for(&kp)).await.is_err());
    }

    #[tokio::test]
    async fn rejects_token_missing_owner_claim() {
        // `owner` has no serde default, so its absence is a parse error even
        // after the signature verifies — this guarantees the sweep always has a
        // hash to compare.
        #[derive(serde::Serialize)]
        struct NoOwner {
            sub: String,
            name: String,
            scp: Vec<String>,
            exp: i64,
            iss: String,
        }
        let kp = test_keypair("kid-1");
        let token = kp.sign(&NoOwner {
            sub: "CHARACTER:EVE:1".into(),
            name: "P".into(),
            scp: vec!["a".into()],
            exp: chrono::Utc::now().timestamp() + 600,
            iss: EVE_ISSUER.into(),
        });
        assert!(verify_and_parse(&token, &cache_for(&kp)).await.is_err());
    }

    #[tokio::test]
    async fn rejects_malformed_token() {
        let kp = test_keypair("kid-1");
        let cache = cache_for(&kp);
        assert!(verify_and_parse("not-a-jwt", &cache).await.is_err());
        assert!(verify_and_parse("only.two", &cache).await.is_err());
    }

    #[tokio::test]
    async fn rejects_token_without_kid() {
        // A token whose header carries no kid cannot select a key.
        let claims = EsiClaims::valid(1, "P", "h");
        let key = jsonwebtoken::EncodingKey::from_secret(b"x");
        let token =
            jsonwebtoken::encode(&jsonwebtoken::Header::new(Algorithm::HS256), &claims, &key)
                .unwrap();
        let kp = test_keypair("kid-1");
        assert!(verify_and_parse(&token, &cache_for(&kp)).await.is_err());
    }
}
