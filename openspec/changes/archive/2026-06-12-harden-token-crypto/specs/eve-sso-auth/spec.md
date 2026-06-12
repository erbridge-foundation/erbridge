# eve-sso-auth — delta for harden-token-crypto

## ADDED Requirements

### Requirement: ESI access-token JWTs are signature-verified
The system SHALL verify every EVE SSO access-token JWT against the SSO JSON Web Key Set before using any of its claims. Verification SHALL cover the signature (against the key matching the token's `kid`), the `exp` expiry, and the `iss` issuer. The JWKS SHALL be fetched at startup from the `jwks_uri` advertised by the SSO discovery document (never hardcoded) and SHALL be refetched when a token presents a `kid` not in the cached set, so SSO key rotation does not require a restart.

A token that fails verification SHALL NOT have its claims used anywhere:
- at the SSO callback, verification failure SHALL produce the same error class as a malformed token exchange (HTTP 502, no account/character/session writes);
- on the background refresh paths (daily sweep, entity-search token refresh), verification failure SHALL be treated as a refresh failure for that character (no token persisted, existing `token_expired` degradation applies).

#### Scenario: Callback rejects a token with an invalid signature
- **WHEN** the token exchange returns an access token whose JWT signature does not verify against the SSO JWKS
- **THEN** the callback responds 502 and writes no account, character, token, or session row

#### Scenario: Sweep treats an unverifiable refreshed token as a refresh failure
- **WHEN** the daily sweep refreshes a character's token and the returned access token fails JWT verification
- **THEN** the character is flagged `token_expired` exactly as if the refresh had been rejected, and the unverified owner claim is not compared or persisted

#### Scenario: Key rotation triggers a JWKS refetch
- **WHEN** a token presents a `kid` absent from the cached JWKS and the SSO publishes a rotated key set containing that `kid`
- **THEN** the backend refetches the JWKS, verification succeeds, and the flow completes without a restart

#### Scenario: JWKS is fetched from discovery, not hardcoded
- **WHEN** the backend starts up
- **THEN** it fetches the JWKS from the `jwks_uri` field of the SSO discovery document and fails startup if the fetch fails
