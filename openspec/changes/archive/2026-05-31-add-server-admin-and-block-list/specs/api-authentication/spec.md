## ADDED Requirements

### Requirement: Bearer authentication rejects accounts owning a blocked character

The bearer branch of API-key authentication (`Authorization: Bearer erb_…`) SHALL reject a request whose resolved account owns at least one blocked character (per the `server-administration` capability's derived account-blocked rule), via a join against `blocked_eve_character`. The rejection SHALL be HTTP 401 with `error.code = "account_blocked"`. The API key row SHALL NOT be deleted.

This check sits alongside the existing soft-deleted rejection on the bearer branch. The session-cookie branch SHALL NOT perform a block-list check: a blocked account has no live session (block deletes all of the account's sessions, per the `server-administration` capability), so the absence of a session is the enforcement — identical to how soft-delete is handled.

#### Scenario: Bearer request for a blocked account is rejected
- **WHEN** a request presents a valid account-scoped API key whose account owns a blocked character
- **THEN** the response is HTTP 401 with `error.code = "account_blocked"` and the key row is not deleted

#### Scenario: Bearer request for a non-blocked account proceeds
- **WHEN** a request presents a valid account-scoped API key whose account owns no blocked character
- **THEN** the request authenticates and proceeds (subject to the existing soft-deleted and scope checks)

#### Scenario: Cookie branch performs no block-list query
- **WHEN** a session-cookie request is authenticated for any account
- **THEN** the cookie branch resolves the session without querying `blocked_eve_character` (block enforcement on the cookie path is via session deletion, not a per-request check)
