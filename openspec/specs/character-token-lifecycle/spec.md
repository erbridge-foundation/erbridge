## Purpose

The lifecycle of an EVE character's ESI tokens and ownership: capturing the SSO `owner` hash on every authentication, the `token_status` state machine (`valid` / `token_expired` / `owner_mismatch`), the daily background token-refresh sweep with its owner-mismatch detection and 7-day idle waterfall, the self-healing of flagged characters on successful re-authentication, the admin character-search and token-state visibility surface, and the audit trail emitted when a character is flagged as transferred.

## Requirements

### Requirement: Owner hash is captured on every successful authentication

The system SHALL parse the `owner` claim from the EVE SSO access-token JWT and persist it as `eve_character.owner_hash` on every successful callback (first link, orphan-claim, re-auth) and on every successful background token refresh, so the stored value reflects the most recently observed claim for that character.

The `owner_hash` column SHALL be nullable. A null stored value means "not yet observed" and SHALL NOT be treated as a transfer; the current authentication records the hash for future comparison.

#### Scenario: First authentication records the owner hash
- **WHEN** a callback links a character that has no existing `eve_character` row
- **THEN** the inserted row's `owner_hash` is set to the `owner` claim from the access-token JWT

#### Scenario: A null stored owner hash is not a transfer
- **WHEN** the sweep or a callback resolves a character whose stored `owner_hash IS NULL`
- **THEN** the presented `owner` claim is recorded and no transfer is flagged

### Requirement: Character token state

Each `eve_character` SHALL carry a `token_status` of exactly one of `valid`, `token_expired`, or `owner_mismatch`. New and freshly authenticated characters are `valid`. The state is advisory (it drives UI guidance) and never terminal.

`owner_mismatch` SHALL be set only on the proven path — a **successful** token refresh whose presented `owner` claim differs from the stored `owner_hash`. A failed refresh SHALL NOT set `owner_mismatch` (no hash can be read from a failure); it sets `token_expired`.

#### Scenario: Default state is valid
- **WHEN** a character row is created or its tokens are written by a normal authentication
- **THEN** its `token_status` is `valid`

### Requirement: Daily token-refresh sweep

The system SHALL run a background task on an approximately 24-hour cadence that, for every character whose `token_status` is not `token_expired` and which holds a refresh token, attempts a token refresh and applies the result:

1. **Refresh succeeds and the owner hash matches the stored hash** → store the rotated tokens and the (unchanged) hash; keep `token_status = valid`.
2. **Refresh succeeds and the owner hash differs from a non-null stored hash** → set `token_status = owner_mismatch`, NULL the credential columns (`encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, empty `scopes`), record the new hash, and emit a character-owner-mismatch audit event.
3. **Refresh fails** → set `token_status = token_expired` and NULL the credential columns.

The sweep SHALL NOT run inside any user request and SHALL NOT depend on any notion of user "presence" or activity.

#### Scenario: Sweep keeps a valid token valid
- **WHEN** the sweep refreshes a character whose refresh succeeds and whose owner hash is unchanged
- **THEN** the rotated tokens are stored and `token_status` remains `valid`

#### Scenario: Sweep flags a transferred character
- **WHEN** the sweep refreshes a character whose refresh succeeds but whose presented owner hash differs from a non-null stored hash
- **THEN** `token_status` becomes `owner_mismatch`, the credential columns are NULLed, the new hash is recorded, and an audit event is emitted recording `eve_character_id` and the owning `account_id`

#### Scenario: Sweep expires a character whose refresh fails
- **WHEN** the sweep attempts to refresh a character and the refresh fails
- **THEN** `token_status` becomes `token_expired` and the credential columns are NULLed

#### Scenario: Sweep skips already-expired characters
- **WHEN** the sweep runs
- **THEN** characters already `token_expired` are not refreshed

### Requirement: Seven-day idle waterfall

When an account's `last_login` is older than 7 days, the sweep SHALL set `token_status = token_expired` and NULL the credential columns for that account's characters that are still `valid`, regardless of refresh-token longevity.

#### Scenario: Idle account's tokens are expired
- **WHEN** the sweep runs and an account's `last_login` is more than 7 days ago
- **THEN** that account's still-`valid` characters become `token_expired` with NULLed credentials

### Requirement: A successful authentication self-heals token state

Any successful login or refresh that presents an `owner` claim matching the current `eve_character.owner_hash` SHALL reset that character's `token_status` to `valid` and restore its tokens, from either `token_expired` or `owner_mismatch`. No `token_status` value is permanent.

#### Scenario: Re-login clears an expired token
- **WHEN** the legitimate owner re-authenticates a `token_expired` character with a matching owner hash
- **THEN** `token_status` returns to `valid` and the fresh tokens are stored

#### Scenario: Matching-hash auth clears a false mismatch
- **WHEN** a character flagged `owner_mismatch` is authenticated and the presented owner hash matches the stored hash (re-acquisition or a prior false positive)
- **THEN** `token_status` returns to `valid` and the fresh tokens are stored

### Requirement: Admin character search and token-state visibility

The admin UI SHALL provide a Characters tab where a server admin can search for a character by name. Selecting a result SHALL open a dialog showing the character's whole account — every character on that account with its `token_status` — so an admin can see and act on a transferred or expired character.

The character listing SHALL support surfacing characters by token state — at minimum, filtering/sorting to find characters whose `token_status` is `token_expired` (and `owner_mismatch`).

#### Scenario: Admin searches and inspects an account
- **WHEN** a server admin searches the Characters tab for a character name and selects a result
- **THEN** a dialog shows that character's account and all of its characters, each with its `token_status`

#### Scenario: Admin filters by expired token state
- **WHEN** a server admin filters or sorts the character listing by token state
- **THEN** characters with `token_status = token_expired` (and `owner_mismatch`) can be surfaced together

### Requirement: A flagged character is recorded in the audit log

When the sweep sets a character to `owner_mismatch`, the system SHALL emit an audit event recording the character's `eve_character_id` and the owning `account_id`.

#### Scenario: Owner mismatch emits an audit event
- **WHEN** the sweep flips a character to `owner_mismatch`
- **THEN** an audit event of the owner-mismatch kind is written with the character's `eve_character_id` and `account_id`
