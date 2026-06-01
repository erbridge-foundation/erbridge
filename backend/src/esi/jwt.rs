//! EVE SSO access-token JWT claim parsing.
//!
//! Both the SSO callback (`handlers/auth.rs`) and the token-refresh path
//! (`esi/token.rs`) receive an access token that is a JWT carrying the
//! character identity. This module owns decoding that JWT's claims so the two
//! call sites share one parser.
//!
//! The token is decoded **without** signature verification — ESI tokens are
//! trusted post-exchange; full JWKS validation is a future hardening step.

use serde::Deserialize;

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

/// Decodes the claims from an EVE SSO access-token JWT without verifying its
/// signature. Returns an error for a malformed token or unparseable claims
/// (which, notably, includes a token missing the required `owner`/`sub`/`name`
/// claims).
pub fn parse_claims(token: &str) -> anyhow::Result<EsiJwtClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("malformed JWT"));
    }
    let payload = parts[1];
    // base64url decode, tolerating both no-pad and padded encodings.
    let padded = match payload.len() % 4 {
        0 => payload.to_string(),
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => return Err(anyhow::anyhow!("invalid base64url padding")),
    };
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| {
            use base64::{Engine, engine::general_purpose::URL_SAFE};
            URL_SAFE.decode(&padded)
        })
        .map_err(|e| anyhow::anyhow!("base64 decode error: {e}"))?;
    serde_json::from_slice(&decoded).map_err(|e| anyhow::anyhow!("JWT claims parse error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    /// Builds a JWT-shaped string (`header.payload.sig`) whose payload is the
    /// given JSON. Signature is irrelevant — the parser does not verify it.
    fn jwt_with_payload(payload_json: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        format!("{header}.{payload}.sig")
    }

    #[test]
    fn parses_all_claims_including_owner() {
        let token = jwt_with_payload(
            r#"{"sub":"CHARACTER:EVE:123","name":"Pilot","owner":"hash-abc","scp":"esi-x.read"}"#,
        );
        let claims = parse_claims(&token).unwrap();
        assert_eq!(claims.sub, "CHARACTER:EVE:123");
        assert_eq!(claims.name, "Pilot");
        assert_eq!(claims.owner, "hash-abc");
        assert_eq!(claims.scp.into_vec(), vec!["esi-x.read".to_string()]);
    }

    #[test]
    fn accepts_scp_array() {
        let token =
            jwt_with_payload(r#"{"sub":"CHARACTER:EVE:1","name":"P","owner":"h","scp":["a","b"]}"#);
        let claims = parse_claims(&token).unwrap();
        assert_eq!(
            claims.scp.into_vec(),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn defaults_missing_scp_to_empty() {
        let token = jwt_with_payload(r#"{"sub":"CHARACTER:EVE:1","name":"P","owner":"h"}"#);
        let claims = parse_claims(&token).unwrap();
        assert!(claims.scp.into_vec().is_empty());
    }

    #[test]
    fn rejects_token_missing_owner() {
        // `owner` has no serde default, so its absence is a parse error — this
        // is what guarantees the sweep always has a hash to compare.
        let token = jwt_with_payload(r#"{"sub":"CHARACTER:EVE:1","name":"P","scp":"a"}"#);
        assert!(parse_claims(&token).is_err());
    }

    #[test]
    fn rejects_malformed_jwt() {
        assert!(parse_claims("not-a-jwt").is_err());
        assert!(parse_claims("only.two").is_err());
    }
}
