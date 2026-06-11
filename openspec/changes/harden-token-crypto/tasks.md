# Tasks — harden-token-crypto

## 1. JWKS fetch + cache

- [ ] 1.1 Add `esi/jwks.rs`: fetch + parse the JWKS from `jwks_uri`, decode keys by `kid` into `jsonwebtoken::DecodingKey`s; cache type with single-flight refetch-on-unknown-`kid` (`tokio::sync::Mutex`)
- [ ] 1.2 Fetch JWKS at startup in `main.rs` (fail-fast like discovery); store the cache in `AppState`
- [ ] 1.3 Unit tests: JWKS parse, unknown-kid refetch (wiremock), refetch failure → verification error

## 2. Verifying parse

- [ ] 2.1 Replace `esi::jwt::parse_claims` with `verify_and_parse(token, &jwks)` validating signature, `exp` (60s leeway), and `iss`; delete the unverified parser
- [ ] 2.2 Update call sites: callback (`handlers/auth.rs`) maps failure to 502; `esi/token.rs::refresh_access_token` maps failure to `None` (refresh failure)
- [ ] 2.3 Test fixtures: RS256 keypair + signed-JWT builder helper for tests; wiremock JWKS endpoint
- [ ] 2.4 Integration tests: callback rejects bad-signature token with 502 and no writes; sweep flags `token_expired` on unverifiable refreshed token

## 3. Key separation

- [ ] 3.1 Add `hkdf` dependency; `jwt_signing_key` becomes HKDF-SHA256(secret, info=`erbridge/session-jwt/v1`); `token_encryption_key` keeps legacy derivation with a doc comment saying why; fix the stale "padded with zeros" doc comment
- [ ] 3.2 Unit tests: derived keys differ from each other and from the raw secret; derivation is deterministic; existing encrypt/decrypt round-trip tests still pass unchanged

## 4. Verification

- [ ] 4.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`
- [ ] 4.2 Regenerate sqlx offline cache if any query changed (none expected): `cargo sqlx prepare -- --all-targets`
- [ ] 4.3 Live smoke test against dev compose: fresh SSO login round-trip succeeds (proves real EVE JWKS verification works); confirm prior sessions are invalidated (one-time logout) and re-login works
