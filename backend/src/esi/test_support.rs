//! Test-only helpers for minting RS256-signed EVE access-token JWTs and the
//! matching JWKS, used by both in-crate unit tests and the integration-test
//! crate (via the `test-support` feature). Not compiled into the production
//! binary.
//!
//! The flow these helpers exercise: generate an RSA keypair → publish its
//! public half as a JWKS document (so [`super::jwks::JwksCache`] can verify
//! against it) → sign a claims payload with the private half (so
//! [`super::jwt::verify_and_parse`] accepts it).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Serialize;

/// A generated RSA keypair tagged with a `kid`, able to sign JWTs and emit the
/// matching JWK.
pub struct TestKeypair {
    pub kid: String,
    private: RsaPrivateKey,
    public: RsaPublicKey,
}

/// Generates a fresh 2048-bit RSA keypair for `kid`. Generation is the slow part
/// of these tests; reuse a keypair across cases where possible.
pub fn test_keypair(kid: &str) -> TestKeypair {
    // `aes_gcm::aead::OsRng` implements the `rand_core 0.6` `CryptoRngCore` that
    // `rsa 0.9` expects (the project's top-level `rand` is 0.9 / `rand_core` 0.9,
    // which is incompatible with `rsa`'s bound).
    let mut rng = aes_gcm::aead::OsRng;
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA test key");
    let public = RsaPublicKey::from(&private);
    TestKeypair {
        kid: kid.to_string(),
        private,
        public,
    }
}

impl TestKeypair {
    /// The base64url-encoded RSA modulus (`n`) and exponent (`e`) for the JWK.
    fn n_e(&self) -> (String, String) {
        let n = URL_SAFE_NO_PAD.encode(self.public.n().to_bytes_be());
        let e = URL_SAFE_NO_PAD.encode(self.public.e().to_bytes_be());
        (n, e)
    }

    /// Signs `claims` as an RS256 JWT whose header carries this keypair's `kid`.
    pub fn sign<C: Serialize>(&self, claims: &C) -> String {
        let pem = self
            .private
            .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
            .unwrap();
        let key = EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        jsonwebtoken::encode(&header, claims, &key).unwrap()
    }
}

/// A ready-to-use [`super::jwks::JwksCache`] holding `kp`'s public key, with no
/// network refetch path. Convenience for the integration-test crate, which
/// needs a cache in an `AppState` but verifies tokens signed by `kp` (or none).
#[cfg(feature = "test-support")]
pub fn jwks_cache_for(kp: &TestKeypair) -> super::jwks::JwksCache {
    let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    super::jwks::JwksCache::from_jwks_body(client, "http://unused", jwks_json(&[kp]).as_bytes())
        .expect("build test JWKS cache")
}

/// Serialises the public halves of the given keypairs into a JWKS document.
pub fn jwks_json(keypairs: &[&TestKeypair]) -> String {
    let keys: Vec<_> = keypairs
        .iter()
        .map(|kp| {
            let (n, e) = kp.n_e();
            serde_json::json!({
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": kp.kid,
                "n": n,
                "e": e,
            })
        })
        .collect();
    serde_json::json!({ "keys": keys }).to_string()
}

/// The standard claim set EVE access tokens carry, with the fields
/// [`super::jwt`] reads. `exp`/`iss` default to a far-future, correct issuer;
/// callers override for negative tests.
#[derive(Serialize)]
pub struct EsiClaims {
    pub sub: String,
    pub name: String,
    pub owner: String,
    pub scp: Vec<String>,
    pub exp: i64,
    pub iss: String,
}

impl EsiClaims {
    /// A valid claim set for character `eve_id` with `owner` hash: non-expired,
    /// correct EVE issuer.
    pub fn valid(eve_id: i64, name: &str, owner: &str) -> Self {
        Self {
            sub: format!("CHARACTER:EVE:{eve_id}"),
            name: name.to_string(),
            owner: owner.to_string(),
            scp: vec!["publicData".to_string()],
            exp: chrono::Utc::now().timestamp() + 1200,
            iss: super::jwt::EVE_ISSUER.to_string(),
        }
    }
}
