use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub const PREFIX: &str = "erb_";

pub fn generate() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let body = URL_SAFE_NO_PAD.encode(bytes);
    format!("{PREFIX}{body}")
}

pub fn hash(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_has_correct_prefix() {
        let key = generate();
        assert!(key.starts_with(PREFIX));
    }

    #[test]
    fn generate_has_correct_length() {
        let key = generate();
        assert_eq!(
            key.len(),
            47,
            "key should be 4 (prefix) + 43 (body) = 47 chars"
        );
    }

    #[test]
    fn generate_body_is_base64url() {
        let key = generate();
        let body = &key[PREFIX.len()..];
        assert_eq!(body.len(), 43);
        assert!(
            body.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "body must be unpadded base64url"
        );
    }

    #[test]
    fn generate_is_unique() {
        let k1 = generate();
        let k2 = generate();
        assert_ne!(k1, k2);
    }

    #[test]
    fn hash_is_sha256_hex() {
        // echo -n "erb_test" | sha256sum
        let expected = {
            let mut h = sha2::Sha256::new();
            h.update(b"erb_test");
            hex::encode(h.finalize())
        };
        assert_eq!(hash("erb_test"), expected);
    }

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(hash("erb_abc"), hash("erb_abc"));
    }

    #[test]
    fn hash_differs_for_different_keys() {
        assert_ne!(hash("erb_aaa"), hash("erb_bbb"));
    }
}
