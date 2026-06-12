# Harden Token Crypto

## Why

A backend security review (2026-06-11) found two cryptographic gaps: (1) EVE SSO access-token JWTs are parsed without signature verification (an acknowledged "future hardening step" in `esi/jwt.rs`) on both the callback and token-refresh paths, and (2) the same 32 raw bytes of `ENCRYPTION_SECRET` serve as both the AES-256-GCM token-encryption key and the HS256 session-JWT signing key — cross-algorithm key reuse with no domain separation.

## What Changes

- Fetch the EVE SSO JWKS (the `jwks_uri` already present in the discovery metadata) and verify every ESI access-token JWT's signature, expiry, and issuer before trusting its claims — at the SSO callback, in the daily token-refresh sweep, and in the entity-search refresh path (all of which funnel through `esi::jwt::parse_claims` today).
- Refetch the JWKS on signature keys we do not recognise (key rotation) rather than only at startup.
- Derive the session-JWT signing key from the root secret via HKDF-SHA256 with a distinct domain-separation label, so the signing key and the token-encryption key are no longer the same bytes. The token-encryption key keeps its current derivation so stored ciphertexts remain decryptable.
- **BREAKING** (operationally): existing session JWTs become invalid at deploy — every user is logged out once and signs back in. Stored ESI tokens are unaffected.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `eve-sso-auth`: ESI access-token JWTs SHALL be signature-verified against the SSO JWKS before their claims are used (new requirement under this capability).

The HKDF key separation changes no observable contract (beyond the one-time session invalidation) and is covered in design.md, not a spec delta.

## Impact

- Backend: `esi/mod.rs` (JWKS fetch + cache), `esi/jwt.rs` (verifying parse replaces unverified parse), `handlers/auth.rs` callback, `esi/token.rs` refresh path, `handlers/crypto.rs` (HKDF derivation), `main.rs` (startup fetch).
- New dependency: `hkdf` crate (RustCrypto, pairs with the existing `sha2`); `jsonwebtoken` already supports RS256/JWKS-style keys.
- Tests: wiremock-served JWKS with a generated test keypair for callback/sweep integration tests; unit tests for key derivation and rotation refetch.
- No database changes. No frontend changes.
