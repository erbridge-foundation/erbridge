## Purpose

The lifecycle of an EVE character's ESI tokens and ownership: capturing the SSO `owner` hash on every authentication, the `token_status` state machine (`valid` / `token_expired` / `owner_mismatch`), the daily background token-refresh sweep with its owner-mismatch detection and 7-day idle waterfall, the self-healing of flagged characters on successful re-authentication, the admin account-roster datagrid that surfaces token state for triage, and the audit trail emitted when a character is flagged as transferred.
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

The admin UI SHALL provide a Characters tab that renders, as a datagrid, every account known to the server — one row per account — so a server admin can see and triage the roster and its token problems without first issuing a search. The grid SHALL read from the already-loaded admin accounts list (`GET /api/v1/admin/accounts`) and SHALL NOT perform a character-name search or any outbound ESI lookup; arbitrary/orphan character lookup is out of scope for this surface.

Each account row SHALL be labelled by the account's main character's name (the character flagged `is_main`); if no character is flagged main, the row SHALL fall back to the first character by name. Each row SHALL surface a roll-up of the account's worst token state (counts of characters whose `token_status` is `token_expired` and `owner_mismatch`) so that token problems are visible without further interaction. A row SHALL expand to reveal every character on that account with its `token_status`.

The grid SHALL support a free-text filter that matches both the account's main name and its alt names (so filtering by an alt name surfaces that alt's account row), account-level status filtering that surfaces accounts having any character whose `token_status` is `token_expired` and/or `owner_mismatch`, and sortable columns.

#### Scenario: Admin sees the account roster without searching

- **WHEN** a server admin opens the Characters tab
- **THEN** the grid lists every account as a row labelled by its main character (or first character by name if none is main), with no search step required

#### Scenario: Admin expands an account to inspect its characters

- **WHEN** a server admin expands an account row
- **THEN** every character on that account is shown with its `token_status`

#### Scenario: Admin surfaces accounts with token problems

- **WHEN** a server admin filters or sorts the grid by token state
- **THEN** accounts having any character whose `token_status` is `token_expired` (and `owner_mismatch`) are surfaced together, and each such account's row shows its problem roll-up without being expanded

#### Scenario: Admin filters by character name

- **WHEN** a server admin types a name fragment into the grid's text filter
- **THEN** rows whose main name or any alt name matches the fragment are shown

### Requirement: A flagged character is recorded in the audit log

When the sweep sets a character to `owner_mismatch`, the system SHALL emit an audit event recording the character's `eve_character_id` and the owning `account_id`.

#### Scenario: Owner mismatch emits an audit event
- **WHEN** the sweep flips a character to `owner_mismatch`
- **THEN** an audit event of the owner-mismatch kind is written with the character's `eve_character_id` and `account_id`

### Requirement: Owner-hash change is acted on at bind time, not only by the sweep

A change in a character's `owner` hash is CCP's canonical transfer signal. In addition to the daily refresh sweep's passive detection (which flags `token_status = owner_mismatch` on a successful refresh whose hash differs), the SSO bind path SHALL act on an owner-hash change at authentication time: when a character authenticates whose presented `owner` hash differs from a non-null stored `owner_hash`, the bind path SHALL detach the character from its prior account and rebind it to the authenticating owner, per the `eve-sso-auth` capability.

This bind-time action and the sweep's flagging are complementary, not contradictory: the sweep flags a transfer it observes on a background refresh of a character still bound to its old account; the bind-time action resolves the transfer the moment the new owner authenticates. A bind-time detach-and-rebind makes the stale `owner_mismatch` flag moot because the character is re-homed and re-stamped `token_status = valid` against the presented (current) hash.

The transfer predicate at bind time SHALL be identical in spirit to the sweep's: a differing hash counts only against a **non-null** stored hash. A null stored hash or an absent presented hash SHALL NOT be treated as a transfer (the current authentication merely records the hash for future comparison).

#### Scenario: Bind-time transfer re-homes the character and clears the stale flag
- **WHEN** a character previously flagged `owner_mismatch` (or still `valid`) authenticates with an `owner` hash differing from its non-null stored hash, while bound to another account
- **THEN** the bind path detaches and rebinds it to the authenticating owner and sets `token_status = valid` against the presented hash, per the `eve-sso-auth` capability

#### Scenario: Null stored hash at bind time is not a transfer
- **WHEN** a character whose stored `owner_hash IS NULL` authenticates
- **THEN** the presented hash is recorded and no detach/transfer occurs

#### Scenario: Sweep flagging remains for unbound-at-rest transfers
- **WHEN** the daily sweep refreshes a character still bound to its old account and observes a differing hash
- **THEN** it still sets `token_status = owner_mismatch` per the existing sweep behaviour (the new owner has not yet authenticated to trigger bind-time re-homing)

