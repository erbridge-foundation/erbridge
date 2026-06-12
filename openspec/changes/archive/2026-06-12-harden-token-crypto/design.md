# Design — harden-token-crypto

## Context

`esi/jwt.rs::parse_claims` base64-decodes the JWT payload and deserialises the claims without checking the signature; the module doc openly defers JWKS validation. Three call sites trust its output: the SSO callback (identity, scopes, owner hash), the sweep's refresh path (owner hash → transfer detection), and the entity-search token refresh. Separately, `handlers/crypto.rs` derives both the AES-256-GCM key and the HS256 session-JWT key as the *same* first 32 bytes of the hex-decoded `ENCRYPTION_SECRET`.

The trust argument today is "the token came over TLS directly from the token endpoint" — true for the callback, but the owner-hash transfer signal is security-relevant enough that defence in depth is warranted, and the JWKS URI is already fetched in the discovery document.

## Goals / Non-Goals

**Goals:**
- No ESI JWT claim is used without signature, expiry, and issuer verification.
- JWKS key rotation is survivable without a restart.
- Distinct keys for distinct cryptographic purposes, derived with explicit domain separation.
- Stored encrypted tokens remain readable across the deploy.

**Non-Goals:**
- Re-encrypting stored tokens under an HKDF-derived AES key. The AES key keeps the legacy derivation; rotating it buys little (same root secret) and costs a data migration.
- Secret rotation tooling (multi-key decrypt). Worth doing someday; out of scope.
- `aud` validation beyond issuer — EVE's audience claim shape (array containing the client_id and `"EVE Online"`) is validated as a follow-up if CCP's contract stabilises; issuer + signature + expiry close the actual gap.

## Decisions

**Verify in `esi/jwt.rs`, keep one funnel.** `parse_claims(token)` becomes `verify_and_parse(token, &jwks)`; the unverified parser is deleted so a future call site cannot quietly choose it. All three call sites already go through this module.

**JWKS cached in `AppState`, refetch on unknown `kid`.** Startup fetches JWKS alongside the discovery document (hard failure, same as discovery — the app cannot authenticate anyone without it). The cache holds the decoded keys keyed by `kid`. On a token whose `kid` is missing from the cache, refetch once (single-flight via a `tokio::sync::Mutex` around the refresh) and retry verification; if still unknown, verification fails. Alternative considered: periodic background refresh — more machinery, and rotation is rare; on-miss refetch is self-correcting exactly when needed.

**Failure mapping preserves existing degradation semantics.** Callback: verification failure → `502 BadGateway` (same class as today's parse failure). Sweep/entity-search refresh: verification failure → treated as a refresh failure (`None`), so the character degrades to `token_expired` rather than erroring the run. No new error surface.

**HKDF-SHA256 for the session-JWT key only.** `jwt_signing_key` becomes `HKDF-SHA256(ikm = decoded secret, salt = none, info = "erbridge/session-jwt/v1")`. `token_encryption_key` keeps the existing `bytes[..32]` derivation, explicitly documented as the legacy-compatible derivation. The `/v1` suffix in the info string leaves room for deliberate future rotation. Alternative considered: HKDF for both + decrypt-reencrypt migration of `eve_character` token columns — rejected as risk without benefit (both keys still derive from the same root secret; separation is the goal, not key freshness).

**Mass logout is accepted, not mitigated.** Session JWTs signed with the old key fail verification after deploy; the extractor already treats that as 401 and the user logs in again. Sessions are 7-day sliding anyway. A dual-key grace window (verify against old + new) was considered and rejected as complexity for a one-time blip on a small deployment.

## Risks / Trade-offs

- [JWKS endpoint outage at startup] App refuses to start. → Same posture as the existing discovery fetch; fail-fast beats limping without the ability to verify identity.
- [JWKS outage during rotation refetch] Verification fails until ESI recovers → callbacks 502, sweep marks characters `token_expired`; the lifecycle self-heals on next successful auth (existing spec behaviour). Acceptable; ESI-down already degrades these paths.
- [Test complexity] Tests must mint RS256-signed JWTs against a wiremock JWKS. → Generate a fixture keypair once (checked-in PEM under `backend/tests/` or generated per-run via the `rsa` crate in dev-dependencies); helper builds signed tokens.
- [Clock skew on `exp`] jsonwebtoken's default leeway (60s) is kept.

## Migration Plan

Single deploy. Users are logged out once (session JWT key change); no data migration; stored tokens unaffected. Rollback is a revert (sessions invalidate once more on the way back).
