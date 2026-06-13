use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use anyhow::{Context, Result, anyhow};
use hkdf::Hkdf;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

/// HKDF `info` label for the session-JWT signing key. The `/v1` suffix is a
/// domain-separation version tag that leaves room for deliberate future key
/// rotation without colliding with previously derived material.
const SESSION_JWT_INFO: &[u8] = b"erbridge/session-jwt/v1";

/// Derives the 32-byte AES-256-GCM key for token encryption from the root secret.
///
/// This is the **legacy** derivation: the first 32 bytes of the hex-decoded
/// secret, taken verbatim. It is deliberately *not* HKDF-derived so that tokens
/// encrypted before the key-separation change remain decryptable — rotating
/// this key would buy nothing (same root secret) at the cost of a data
/// migration over every stored token. See `design.md`.
pub fn token_encryption_key(encryption_secret: &str) -> Result<Vec<u8>> {
    hex_decode_secret(encryption_secret)
}

/// Derives the HS256 signing key for session JWTs from the root secret via
/// HKDF-SHA256 with a distinct domain-separation label, so the signing key and
/// the token-encryption key are no longer the same bytes (the cross-algorithm
/// key-reuse gap closed by harden-token-crypto). No salt: the root secret is
/// already high-entropy random hex, and a fixed `info` label provides the
/// separation we need.
pub fn jwt_signing_key(encryption_secret: &str) -> Result<Vec<u8>> {
    let ikm = hex_decode_secret(encryption_secret)?;
    let hkdf = Hkdf::<Sha256>::new(None, &ikm);
    let mut okm = [0u8; 32];
    hkdf.expand(SESSION_JWT_INFO, &mut okm)
        .map_err(|e| anyhow!("HKDF expand failed: {e}"))?;
    Ok(okm.to_vec())
}

fn hex_decode_secret(secret: &str) -> Result<Vec<u8>> {
    let bytes = hex::decode(secret).context("ENCRYPTION_SECRET must be a valid hex string")?;
    if bytes.len() < 32 {
        return Err(anyhow!(
            "ENCRYPTION_SECRET must be at least 32 bytes (64 hex chars)"
        ));
    }
    Ok(bytes[..32].to_vec())
}

/// Encrypts a plaintext token, returning `nonce (12 bytes) || ciphertext+tag` as bytes.
pub fn encrypt_token(plaintext: &str, key_bytes: &[u8]) -> Result<Vec<u8>> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("encryption failed: {e}"))?;

    let mut output = nonce.to_vec();
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypts a token previously produced by `encrypt_token`. Used by the daily
/// token-refresh sweep and the entity-search path to recover stored ESI refresh
/// tokens; the encrypt side is used by the SSO callback.
pub fn decrypt_token(ciphertext_with_nonce: &[u8], key_bytes: &[u8]) -> Result<String> {
    if ciphertext_with_nonce.len() < 12 {
        return Err(anyhow!("ciphertext too short"));
    }
    let (nonce_bytes, ciphertext) = ciphertext_with_nonce.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("decryption failed: {e}"))?;

    String::from_utf8(plaintext).context("decrypted token is not valid UTF-8")
}

#[derive(Serialize, Deserialize)]
struct SessionClaims {
    sub: String,
    exp: i64,
}

const SESSION_JWT_LIFETIME_SECONDS: i64 = 7 * 24 * 60 * 60;

/// Signs a session ID as an HS256 JWT with `exp = now() + 7 days` for use in
/// the session cookie. Re-issued on every authenticated request so the
/// browser-side cookie lifetime tracks the server-side `session.expires_at`.
pub fn sign_session_jwt(session_id: &str, key_bytes: &[u8]) -> Result<String> {
    let exp = chrono::Utc::now().timestamp() + SESSION_JWT_LIFETIME_SECONDS;
    let claims = SessionClaims {
        sub: session_id.to_string(),
        exp,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(key_bytes),
    )
    .context("failed to sign session JWT")
}

/// Verifies and extracts the session ID from an HS256 JWT. Rejects expired tokens.
pub fn verify_session_jwt(token: &str, key_bytes: &[u8]) -> Result<String> {
    let validation = Validation::new(Algorithm::HS256);

    let data = decode::<SessionClaims>(token, &DecodingKey::from_secret(key_bytes), &validation)
        .context("invalid session JWT")?;

    Ok(data.claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        hex::decode("0".repeat(64)).unwrap()
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = "my_access_token_value";
        let encrypted = encrypt_token(plaintext, &key).unwrap();
        let decrypted = decrypt_token(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_produces_different_nonces() {
        let key = test_key();
        let enc1 = encrypt_token("same", &key).unwrap();
        let enc2 = encrypt_token("same", &key).unwrap();
        // nonces should differ (fresh per call)
        assert_ne!(enc1[..12], enc2[..12]);
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let key = test_key();
        let mut encrypted = encrypt_token("token", &key).unwrap();
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xff;
        assert!(decrypt_token(&encrypted, &key).is_err());
    }

    #[test]
    fn jwt_sign_verify_roundtrip() {
        let key = test_key();
        let session_id = "my-session-id";
        let token = sign_session_jwt(session_id, &key).unwrap();
        let extracted = verify_session_jwt(&token, &key).unwrap();
        assert_eq!(extracted, session_id);
    }

    #[test]
    fn jwt_verify_rejects_wrong_key() {
        let key = test_key();
        let other_key = hex::decode("f".repeat(64)).unwrap();
        let token = sign_session_jwt("sess", &key).unwrap();
        assert!(verify_session_jwt(&token, &other_key).is_err());
    }

    #[test]
    fn derived_keys_differ_from_each_other_and_from_raw_secret() {
        let secret = "a".repeat(64);
        let raw = hex::decode(&secret).unwrap();
        let enc = token_encryption_key(&secret).unwrap();
        let sign = jwt_signing_key(&secret).unwrap();
        // The encryption key is the legacy raw-bytes derivation...
        assert_eq!(enc, raw[..32]);
        // ...while the signing key is HKDF-expanded and so differs from both the
        // encryption key and the raw secret bytes.
        assert_ne!(sign, enc);
        assert_ne!(sign, raw[..32].to_vec());
        assert_eq!(sign.len(), 32);
    }

    #[test]
    fn jwt_signing_key_derivation_is_deterministic() {
        let secret = "b".repeat(64);
        assert_eq!(
            jwt_signing_key(&secret).unwrap(),
            jwt_signing_key(&secret).unwrap()
        );
    }

    #[test]
    fn hex_decode_secret_rejects_short_key() {
        let short = hex::encode([0u8; 16]);
        assert!(token_encryption_key(&short).is_err());
    }

    #[test]
    fn hex_decode_secret_rejects_invalid_hex() {
        assert!(token_encryption_key("not-hex!!").is_err());
    }
}
