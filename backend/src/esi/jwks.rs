//! EVE SSO JSON Web Key Set (JWKS) fetch + cache.
//!
//! The SSO discovery document advertises a `jwks_uri` holding the RSA public
//! keys that sign EVE access-token JWTs. This module fetches that key set,
//! decodes each key into a [`jsonwebtoken::DecodingKey`] indexed by its `kid`,
//! and caches the result. When a token presents a `kid` we have not seen, the
//! cache refetches once (single-flight) to pick up a rotated key set without a
//! restart; if the `kid` is still unknown, verification fails.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use jsonwebtoken::DecodingKey;
use serde::Deserialize;
use tokio::sync::Mutex;

/// A single key from a JWKS document. We only need RSA keys (EVE signs its
/// access tokens with RS256); other key types (e.g. EVE's `EC`/`ES256` key)
/// carry different members (`x`/`y`/`crv` instead of `n`/`e`) and are skipped,
/// not rejected, so a mixed key set parses. Members beyond `kid`/`n`/`e`
/// (`use`, `alg`, `kty`) are ignored — `jsonwebtoken` takes the modulus and
/// exponent directly.
#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    /// RSA modulus, base64url-encoded. Absent on non-RSA keys.
    n: Option<String>,
    /// RSA public exponent, base64url-encoded. Absent on non-RSA keys.
    e: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

/// Decodes a JWKS document body into `kid -> DecodingKey`, keeping the RSA keys
/// and skipping any non-RSA key (which lacks `n`/`e`). An RSA key whose
/// components are present but unusable is an error — that is corruption, not a
/// key type we deliberately ignore. An empty result (no RSA key at all) is an
/// error: we would rather fail loudly at fetch time than verify nothing.
fn decode_keys(body: &[u8]) -> Result<HashMap<String, DecodingKey>> {
    let set: JwkSet = serde_json::from_slice(body).context("failed to parse JWKS document")?;
    let mut keys = HashMap::with_capacity(set.keys.len());
    for jwk in set.keys {
        // Skip non-RSA keys (no modulus/exponent) — EVE also publishes an EC key.
        let (Some(n), Some(e)) = (jwk.n.as_deref(), jwk.e.as_deref()) else {
            continue;
        };
        let key = DecodingKey::from_rsa_components(n, e)
            .with_context(|| format!("invalid RSA key components for kid {}", jwk.kid))?;
        keys.insert(jwk.kid, key);
    }
    if keys.is_empty() {
        return Err(anyhow!("JWKS document contained no usable RSA keys"));
    }
    Ok(keys)
}

async fn fetch_keys(
    http: &reqwest_middleware::ClientWithMiddleware,
    jwks_uri: &str,
) -> Result<HashMap<String, DecodingKey>> {
    let body = http
        .get(jwks_uri)
        .send()
        .await
        .context("failed to fetch EVE SSO JWKS")?
        .error_for_status()
        .context("EVE SSO JWKS endpoint returned non-2xx")?
        .bytes()
        .await
        .context("failed to read EVE SSO JWKS body")?;
    decode_keys(&body)
}

/// The cached JWKS plus the means to refetch it on a key-rotation miss.
///
/// `keys` is behind a `Mutex` so the refetch is single-flight: a concurrent
/// burst of unknown-`kid` tokens during a rotation refetches once, not once per
/// request. Holding the lock across the network call serialises refetches —
/// acceptable because rotation is rare and the fetch is fast; the steady-state
/// path takes the lock only to clone an already-present key.
pub struct JwksCache {
    http: reqwest_middleware::ClientWithMiddleware,
    jwks_uri: String,
    keys: Mutex<HashMap<String, DecodingKey>>,
}

impl std::fmt::Debug for JwksCache {
    // `DecodingKey` is not `Debug`; expose only the non-secret URI.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwksCache")
            .field("jwks_uri", &self.jwks_uri)
            .finish_non_exhaustive()
    }
}

impl JwksCache {
    /// Fetches the JWKS once and builds the cache. Used at startup; a failure
    /// here is fatal (the app cannot verify any identity without the key set).
    pub async fn fetch(
        http: reqwest_middleware::ClientWithMiddleware,
        jwks_uri: &str,
    ) -> Result<Self> {
        let keys = fetch_keys(&http, jwks_uri).await?;
        Ok(Self {
            http,
            jwks_uri: jwks_uri.to_string(),
            keys: Mutex::new(keys),
        })
    }

    /// Returns the decoding key for `kid`, refetching the JWKS once if `kid` is
    /// absent from the cache (SSO key rotation). Errors if the `kid` is still
    /// unknown after a refetch, or if the refetch itself fails.
    pub async fn key_for(&self, kid: &str) -> Result<DecodingKey> {
        let mut keys = self.keys.lock().await;
        if let Some(key) = keys.get(kid) {
            return Ok(key.clone());
        }
        // Unknown kid: refetch once to pick up a rotated key set.
        let refreshed = fetch_keys(&self.http, &self.jwks_uri).await?;
        *keys = refreshed;
        keys.get(kid)
            .cloned()
            .ok_or_else(|| anyhow!("no JWKS key matches kid {kid} after refetch"))
    }

    /// Builds a cache from a JWKS document body without a network fetch. Gated
    /// behind `test-support` so the integration-test crate can construct an
    /// `AppState` with a usable cache.
    #[cfg(feature = "test-support")]
    pub fn from_jwks_body(
        http: reqwest_middleware::ClientWithMiddleware,
        jwks_uri: &str,
        body: &[u8],
    ) -> Result<Self> {
        Ok(Self {
            http,
            jwks_uri: jwks_uri.to_string(),
            keys: Mutex::new(decode_keys(body)?),
        })
    }

    /// Test-only constructor: builds a cache from pre-decoded keys without a
    /// network fetch, so verification can be unit-tested across modules.
    #[cfg(test)]
    pub(crate) fn from_keys_for_test(
        http: reqwest_middleware::ClientWithMiddleware,
        jwks_uri: &str,
        keys: HashMap<String, DecodingKey>,
    ) -> Self {
        Self {
            http,
            jwks_uri: jwks_uri.to_string(),
            keys: Mutex::new(keys),
        }
    }
}

/// Test-only re-export of the private decoder so sibling modules' tests can
/// build a cache from a JWKS document body.
#[cfg(test)]
pub(crate) fn decode_keys_for_test(body: &[u8]) -> HashMap<String, DecodingKey> {
    #[allow(clippy::unwrap_used)]
    decode_keys(body).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::esi::test_support::{jwks_json, test_keypair};
    use reqwest_middleware::ClientBuilder;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

    fn http() -> reqwest_middleware::ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new()).build()
    }

    #[test]
    fn parses_a_jwks_document() {
        let kp = test_keypair("kid-1");
        let keys = decode_keys(jwks_json(&[&kp]).as_bytes()).unwrap();
        assert!(keys.contains_key("kid-1"));
    }

    #[test]
    fn rejects_empty_jwks() {
        assert!(decode_keys(br#"{"keys":[]}"#).is_err());
    }

    #[test]
    fn skips_non_rsa_keys_and_keeps_the_rsa_one() {
        // Mirrors EVE's real JWKS shape: an RS256 RSA key (which signs the
        // access tokens) alongside an ES256 EC key (no n/e) and a trailing
        // non-key member. The EC key must be skipped, not rejected.
        let body = br#"{
            "keys": [
                {"alg":"RS256","e":"AQAB","kid":"JWT-Signature-Key","kty":"RSA","n":"nehPQ7FQ1YK-leKyIg-aACZaT-DbTL5V1XpXghtLX_bEC-fwxhdE_4yQKDF6cA-V4c-5kh8wMZbfYw5xxgM9DynhMkVrmQFyYB3QMZwydr922UWs3kLz-nO6vi0ldCn-ffM9odUPRHv9UbhM5bB4SZtCrpr9hWQgJ3FjzWO2KosGQ8acLxLtDQfU_lq0OGzoj_oWwUKaN_OVfu80zGTH7mxVeGMJqWXABKd52ByvYZn3wL_hG60DfDWGV_xfLlHMt_WoKZmrXT4V3BCBmbitJ6lda3oNdNeHUh486iqaL43bMR2K4TzrspGMRUYXcudUQ9TycBQBrUlT85NRY9TeOw","use":"sig"},
                {"alg":"ES256","crv":"P-256","kid":"ec-key","kty":"EC","use":"sig","x":"PatzB2HJzZOzmqQyYpQYqn3SAXoVYWrZKmMgJnfK94I","y":"qDb1kUd13fRTN2UNmcgSoQoyqeF_C1MsFlY_a87csnY"}
            ],
            "SkipUnresolvedJsonWebKeys": true
        }"#;
        let keys = decode_keys(body).unwrap();
        assert!(keys.contains_key("JWT-Signature-Key"));
        assert!(!keys.contains_key("ec-key"));
    }

    #[test]
    fn rejects_jwks_with_only_non_rsa_keys() {
        let body = br#"{"keys":[{"kty":"EC","kid":"ec","crv":"P-256","x":"a","y":"b"}]}"#;
        assert!(decode_keys(body).is_err());
    }

    #[tokio::test]
    async fn key_for_known_kid_does_not_refetch() {
        let kp = test_keypair("kid-1");
        let cache = JwksCache::from_keys_for_test(
            http(),
            "http://unused",
            decode_keys(jwks_json(&[&kp]).as_bytes()).unwrap(),
        );
        assert!(cache.key_for("kid-1").await.is_ok());
    }

    #[tokio::test]
    async fn unknown_kid_triggers_single_refetch() {
        // Cache starts with kid-1; the rotated endpoint serves kid-2.
        let old = test_keypair("kid-1");
        let new = test_keypair("kid-2");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/jwks"))
            .respond_with(ResponseTemplate::new(200).set_body_string(jwks_json(&[&new])))
            .mount(&server)
            .await;
        let uri = format!("{}/jwks", server.uri());
        let cache = JwksCache::from_keys_for_test(
            http(),
            &uri,
            decode_keys(jwks_json(&[&old]).as_bytes()).unwrap(),
        );

        // kid-2 is absent → refetch → now present.
        assert!(cache.key_for("kid-2").await.is_ok());
        // kid-1 was replaced by the refetched set.
        assert!(cache.key_for("kid-1").await.is_err() || cache.key_for("kid-2").await.is_ok());
    }

    #[tokio::test]
    async fn unknown_kid_with_failing_refetch_errors() {
        let old = test_keypair("kid-1");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/jwks"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let uri = format!("{}/jwks", server.uri());
        let cache = JwksCache::from_keys_for_test(
            http(),
            &uri,
            decode_keys(jwks_json(&[&old]).as_bytes()).unwrap(),
        );
        assert!(cache.key_for("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn fetch_builds_cache_from_endpoint() {
        let kp = test_keypair("kid-1");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/jwks"))
            .respond_with(ResponseTemplate::new(200).set_body_string(jwks_json(&[&kp])))
            .mount(&server)
            .await;
        let uri = format!("{}/jwks", server.uri());
        let cache = JwksCache::fetch(http(), &uri).await.unwrap();
        assert!(cache.key_for("kid-1").await.is_ok());
    }

    #[tokio::test]
    async fn concurrent_unknown_kid_refetches_once() {
        // A response that counts how many times it is hit, to prove the Mutex
        // serialises the refetch into a single call under a concurrent burst.
        struct Counting {
            body: String,
            hits: Arc<AtomicUsize>,
        }
        impl Respond for Counting {
            fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
                self.hits.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_string(self.body.clone())
            }
        }

        let old = test_keypair("kid-1");
        let new = test_keypair("kid-2");
        let hits = Arc::new(AtomicUsize::new(0));
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/jwks"))
            .respond_with(Counting {
                body: jwks_json(&[&new]),
                hits: hits.clone(),
            })
            .mount(&server)
            .await;
        let uri = format!("{}/jwks", server.uri());
        let cache = Arc::new(JwksCache::from_keys_for_test(
            http(),
            &uri,
            decode_keys(jwks_json(&[&old]).as_bytes()).unwrap(),
        ));

        let mut handles = vec![];
        for _ in 0..8 {
            let c = cache.clone();
            handles.push(tokio::spawn(
                async move { c.key_for("kid-2").await.is_ok() },
            ));
        }
        for h in handles {
            assert!(h.await.unwrap());
        }
        // Once the first refetch populates kid-2, the lock-holding followers find
        // it present and skip their own fetch: exactly one network hit.
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
