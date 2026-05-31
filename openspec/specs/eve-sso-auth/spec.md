## Purpose

EVE ESI OAuth2 authentication flow — `GET /auth/login`, `GET /auth/callback`, `GET /auth/logout`, `GET /auth/characters/add` — plus the Postgres-backed session store, sliding 7-day expiry, session-cookie JWT, ESI scope handling, and ESI discovery-document caching that support it.

## Requirements

### Requirement: Login redirects to EVE SSO
The system SHALL redirect the browser to the EVE ESI authorization endpoint when `GET /auth/login` is requested. The authorization URL SHALL be derived from the ESI discovery document fetched at startup, never hardcoded. The redirect SHALL include all required OAuth2 parameters: `response_type=code`, `client_id`, `redirect_uri` (`{APP_URL}/auth/callback`), `scope` (the full required scope list), and a `state` parameter for CSRF protection.

`GET /auth/login` and `GET /auth/characters/add` SHALL accept an OPTIONAL `?return_to=<path>` query parameter. The value SHALL be validated as a same-origin path: it MUST start with a single `/`, MUST NOT start with `//` or `/\\` (which browsers may interpret as a scheme-relative URL), and MUST NOT contain `\r` or `\n`. The validated value SHALL be stashed alongside the CSRF state in the session's in-flight OAuth2 record. The OAuth2 callback handler SHALL redirect the browser to this path on success. If `return_to` is absent or fails validation, the callback SHALL redirect to `/`.

#### Scenario: Unauthenticated user visits login
- **WHEN** a browser requests `GET /auth/login`
- **THEN** the backend responds with HTTP 302 redirecting to the EVE SSO authorization URL with all required query parameters

#### Scenario: Authorization URL is not hardcoded
- **WHEN** the backend starts up
- **THEN** it fetches `https://login.eveonline.com/.well-known/oauth-authorization-server` and uses `authorization_endpoint` from the response for all login redirects

#### Scenario: Login stashes a valid return_to and the callback honours it
- **WHEN** a browser requests `GET /auth/login?return_to=/characters` and subsequently completes the SSO flow
- **THEN** the callback redirects the browser to `/characters` instead of `/`

#### Scenario: Add-character stashes a valid return_to
- **WHEN** an authenticated browser requests `GET /auth/characters/add?return_to=/characters` and subsequently completes the SSO flow
- **THEN** the callback redirects the browser to `/characters`

#### Scenario: Absent return_to defaults to /
- **WHEN** the SSO flow completes with no `return_to` stashed
- **THEN** the callback redirects the browser to `/`

#### Scenario: Off-origin return_to is rejected
- **WHEN** a browser requests `GET /auth/login?return_to=https://evil.example.com/`
- **THEN** the value is rejected during validation and the callback redirects to `/` (the open-redirect vector is closed)

#### Scenario: Scheme-relative return_to is rejected
- **WHEN** a browser requests `GET /auth/login?return_to=//evil.example.com/`
- **THEN** the value is rejected during validation and the callback redirects to `/`

### Requirement: OAuth2 callback exchanges code and persists character
The system SHALL handle `GET /auth/callback` by exchanging the authorization code for access and refresh tokens via the ESI token endpoint, parsing the JWT access token to extract `eve_character_id`, `name`, and `scp` (the granted ESI scopes), and fetching `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name` from ESI public-info endpoints. The corp and alliance **names** are persisted alongside the IDs on the `eve_character` row so that downstream reads (notably `GET /api/v1/me`) do not need to call ESI.

The `scp` claim in the EVE access-token JWT MAY be either a single string (when one scope was granted) or an array of strings (when multiple scopes were granted). Implementations SHALL accept both shapes and normalise to a `TEXT[]` for persistence in `eve_character.scopes`.

On success the backend SHALL:

1. Look up `eve_character` by `eve_character_id`:
   - **If no row exists**: create a new `account` row (when not in add-character mode) or use the current session's `account_id` (add-character mode); insert a new `eve_character` row with `account_id` set, encrypted access and refresh tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, and the public-info fields.
   - **If an orphan row exists** (`account_id IS NULL`): claim it by setting `account_id` to the resolved account, writing the encrypted tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, and refreshing the public-info fields.
   - **If a row exists with `account_id` set**: overwrite `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, `scopes`, refresh public-info fields, bump `updated_at`. The row's `account_id` is the session's account.
2. If the resolved account has no character flagged `is_main = TRUE` after the row is written/claimed (this is true for any account that just gained its first character, including via orphan-claim), the same transaction SHALL set `is_main = TRUE` on the just-written character. There SHALL always be exactly one main per account that has any characters.
3. If the resolved `account.status` is `'soft_deleted'`, atomically reactivate it: `status = 'active'`, `delete_requested_at = NULL`.
4. Establish or update a session: insert (or update, in add-character mode) a row in the Postgres `session` table keyed by session ID, with `account_id` set to the resolved account, `csrf_state` and `add_character_mode` carried from the in-flight OAuth2 record, `created_at = now()` for new rows, `last_seen_at = now()`, and `expires_at = now() + interval '7 days'`. The row does NOT hold token material; tokens live only in `eve_character`.
5. Set the session cookie (`httpOnly`, `SameSite=Lax`) carrying a signed HS256 JWT whose `exp` claim is 7 days out, and redirect to the validated `return_to` path stashed in the in-flight OAuth2 record, or to `/` when none was stashed.

#### Scenario: Valid callback creates session and redirects home
- **WHEN** EVE SSO redirects to `/auth/callback` with a valid `code` and matching `state`
- **THEN** the backend exchanges the code for tokens, persists them encrypted in `eve_character`, inserts a row in the `session` table pointing to the resolved account with `expires_at = now() + interval '7 days'`, sets a session cookie whose JWT carries a matching `exp`, and redirects the browser to `/`

#### Scenario: Orphan character is claimed on first login
- **WHEN** the callback resolves an `eve_character_id` that exists with `account_id = NULL`
- **THEN** the existing row's `account_id` is set to the (possibly newly-created) account; no duplicate row is created

#### Scenario: Login reactivates a soft-deleted account
- **WHEN** the callback resolves to an account with `status = 'soft_deleted'`
- **THEN** the same transaction that writes the tokens also sets `status = 'active'` and `delete_requested_at = NULL`

#### Scenario: First linked character is promoted to main
- **WHEN** the callback writes a character to an account that has no existing `is_main = TRUE` row
- **THEN** the same transaction sets `is_main = TRUE` on the just-written character

#### Scenario: Subsequent character does not displace existing main
- **WHEN** the callback writes a new character to an account that already has a different character with `is_main = TRUE`
- **THEN** the existing main is unchanged and the new character is inserted with `is_main = FALSE`

#### Scenario: Session row holds no token material
- **WHEN** any row in the `session` table is inspected
- **THEN** the row holds only `session_id`, `account_id`, `csrf_state`, `add_character_mode`, `created_at`, `last_seen_at`, and `expires_at`; it does NOT hold the access or refresh token

#### Scenario: Invalid or missing state parameter
- **WHEN** `/auth/callback` is called with a missing or mismatched `state` parameter
- **THEN** the backend responds with HTTP 400 and does not create a session

#### Scenario: Token exchange failure
- **WHEN** the ESI token endpoint returns an error for the provided code
- **THEN** the backend responds with HTTP 502 and does not create a session

### Requirement: Logout clears session
The system SHALL handle `GET /auth/logout` by deleting the session row from the Postgres `session` table and clearing the session cookie. The response SHALL redirect to `/`.

#### Scenario: Authenticated user logs out
- **WHEN** a browser with a valid session cookie requests `GET /auth/logout`
- **THEN** the row in the `session` table for that session ID is deleted, the session cookie is cleared, and the browser is redirected to `/`

#### Scenario: Unauthenticated user requests logout
- **WHEN** a browser with no session cookie requests `GET /auth/logout`
- **THEN** the backend redirects to `/` without error

### Requirement: Add-character links additional character to session
The system SHALL handle `GET /auth/characters/add` by redirecting to the EVE SSO authorization endpoint in add-character mode. On callback, the new character's tokens SHALL be added to the existing session rather than creating a new session. The endpoint SHALL also accept the OPTIONAL `?return_to=<path>` query parameter described under "Login redirects to EVE SSO"; the callback honours it on completion.

#### Scenario: Authenticated user adds a second character
- **WHEN** a browser with a valid session cookie requests `GET /auth/characters/add`
- **THEN** the backend redirects to EVE SSO; on successful callback, the new character is appended to the session's character list and the browser is redirected to `/`

#### Scenario: Unauthenticated user attempts add-character
- **WHEN** a browser with no session cookie requests `GET /auth/characters/add`
- **THEN** the backend responds with HTTP 401

### Requirement: Session cookie does not expose token material
The session cookie SHALL contain only an encrypted, signed session ID — no access tokens, refresh tokens, character IDs, or other EVE credential material. The cookie SHALL be `httpOnly` and `SameSite=Lax`.

#### Scenario: Session cookie inspection reveals no token data
- **WHEN** a session is established after login
- **THEN** the `Set-Cookie` header value contains only an opaque session ID; access and refresh tokens are absent from all response headers and cookie values

### Requirement: Sessions survive backend restarts
The session store SHALL be backed by Postgres so that session rows persist across backend process restarts. A browser holding a valid, unexpired session cookie SHALL remain authenticated across a backend restart without re-running the EVE SSO flow.

#### Scenario: Session survives restart
- **WHEN** a user logs in, the backend process is restarted, and the same browser makes an authenticated request before the session's `expires_at`
- **THEN** the request is authenticated against the persisted `session` row and succeeds; no re-login is required

### Requirement: Sessions expire after 7 days of inactivity (sliding)
Each session row SHALL carry an `expires_at` timestamp. On creation, `expires_at` SHALL be set to `now() + interval '7 days'`. On every successful authenticated request that resolves via the session cookie, the backend SHALL atomically advance `last_seen_at = now()` and `expires_at = now() + interval '7 days'` for the matched row, gated by `expires_at > now()`. A row whose `expires_at` is in the past SHALL be treated as if it does not exist: cookie-authenticated requests resolving to it SHALL respond with HTTP 401.

API-key authenticated requests (bearer `erb_…`) do NOT touch the session table and do NOT extend any session.

#### Scenario: Active session is refreshed on each request
- **WHEN** a cookie-authenticated request succeeds for session `S`
- **THEN** `S.last_seen_at` is updated to `now()` and `S.expires_at` is updated to `now() + interval '7 days'` in the same database round-trip

#### Scenario: Session past its expiry is rejected
- **WHEN** a request arrives with a cookie whose session row has `expires_at < now()`
- **THEN** the backend responds with HTTP 401 and does not extend the row

#### Scenario: Idle session expires after 7 days
- **WHEN** a session is created at time `T` and no requests using it arrive before `T + 7 days`
- **THEN** any request arriving at `T + 7 days + ε` with that session's cookie is rejected with HTTP 401

#### Scenario: API-key request does not extend session
- **WHEN** an authenticated `Authorization: Bearer erb_…` request arrives for an account that also has an active session row
- **THEN** the session row's `last_seen_at` and `expires_at` are unchanged

### Requirement: Session cookie JWT is refreshed on each authenticated request
On every cookie-authenticated request whose session row was successfully refreshed (per the sliding-expiry requirement), the response SHALL include a fresh `Set-Cookie` header carrying a newly-signed session JWT whose `exp` claim is 7 days from the response time. The session ID and signing key SHALL be unchanged from the prior cookie; only the `exp` is advanced.

If the session row is missing or expired, no refreshed cookie SHALL be set (the request fails authentication anyway).

#### Scenario: Cookie is reissued on successful auth
- **WHEN** a cookie-authenticated request succeeds
- **THEN** the response carries a `Set-Cookie` header with a fresh JWT whose `exp` is approximately `now() + 7 days`

#### Scenario: Cookie is not reissued on auth failure
- **WHEN** a request arrives with a cookie whose session is expired or missing
- **THEN** the response does NOT include a fresh session cookie (and the response is HTTP 401)

### Requirement: Required ESI scopes are requested
The OAuth2 authorization request SHALL include exactly the following scopes: `esi-location.read_location.v1`, `esi-location.read_ship_type.v1`, `esi-location.read_online.v1`, `esi-search.search_structures.v1`, `esi-ui.write_waypoint.v1`.

#### Scenario: Login redirect includes all required scopes
- **WHEN** `GET /auth/login` is called
- **THEN** the redirect URL's `scope` parameter contains all five required ESI scope strings

### Requirement: ESI discovery document is fetched once at startup
The backend SHALL fetch the EVE SSO well-known discovery document (`https://login.eveonline.com/.well-known/oauth-authorization-server`) exactly once at process startup and cache `EsiMetadata` (containing `authorization_endpoint`, `token_endpoint`, `jwks_uri`) in Axum application state. If the fetch fails at startup, the process SHALL exit with a clear error.

The implementation SHALL use the following code verbatim in `backend/src/esi/mod.rs`:

```rust
use anyhow::{Context, Result};
use serde::Deserialize;

const WELL_KNOWN_URL: &str =
    "https://login.eveonline.com/.well-known/oauth-authorization-server";

/// Metadata returned by the EVE SSO `/.well-known/oauth-authorization-server` endpoint.
#[derive(Clone, Debug, Deserialize)]
pub struct EsiMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
}

pub async fn discover(http: &reqwest::Client) -> Result<EsiMetadata> {
    http.get(WELL_KNOWN_URL)
        .send()
        .await
        .context("failed to fetch EVE SSO discovery document")?
        .error_for_status()
        .context("EVE SSO discovery document returned non-2xx")?
        .json::<EsiMetadata>()
        .await
        .context("failed to parse EVE SSO discovery document")
}
```

`EsiMetadata` SHALL be stored in Axum application state alongside the session store.

#### Scenario: Discovery succeeds at startup
- **WHEN** the backend process starts with network access to EVE SSO
- **THEN** `EsiMetadata` is populated in AppState and no further requests to the well-known URL are made

#### Scenario: Discovery fails at startup
- **WHEN** the well-known endpoint is unreachable at startup
- **THEN** the backend process exits with a non-zero status code and logs the error

### Requirement: SSO callback emits audit events for account-lifecycle transitions

The OAuth2 callback handler SHALL emit audit events (per the `audit-log` capability) into the same transaction that performs each of the following actions. Emissions SHALL occur *after* the `promote_if_no_main` step so that the actor-character snapshot resolves correctly for any subsequent audit emission in the same transaction.

Concretely, for each transaction processed by `GET /auth/callback`:

1. If the callback creates a new `account` row (the first-character flow), it SHALL emit `AccountRegistered { account_id, eve_character_id, character_name }` using `acting_as = Some(ActingCharacter { eve_character_id, name: character_name })` and `actor_account_id = None` (no session exists yet).
2. If the callback claims a pre-existing orphan `eve_character` row (one with `account_id IS NULL`), it SHALL emit `OrphanCharacterClaimed { account_id, eve_character_id, character_name }` with the same `acting_as` / `actor_account_id = None` pattern.
3. If the callback reactivates a soft-deleted account (per the existing "Login reactivates a soft-deleted account" scenario), it SHALL emit `AccountReactivated { account_id }` with `acting_as = Some(ActingCharacter { … })` and `actor_account_id = None` (the session is being re-established within this transaction).
4. If the callback promotes the just-resolved account to server admin via the first-account bootstrap rule (per the existing `resolve_or_create` behaviour where the very first account in the system gets `is_server_admin = TRUE`), it SHALL emit `ServerAdminGranted { account_id, source: ServerAdminGrantSource::FirstAccountBootstrap }` with `actor_account_id = None` and `acting_as = Some(...)`.
5. If the callback is in add-character mode (the `/auth/characters/add` flow with an authenticated session), and a new `eve_character` row is created for the existing account, it SHALL emit `CharacterAdded { account_id, eve_character_id, character_name }` with `actor_account_id = Some(account_id)` and `acting_as = None`. (In this flow the account exists, has a main, and the session predates the SSO redirect.)
6. If add-character mode claims an existing orphan rather than creating a fresh row, it SHALL emit `OrphanCharacterClaimed { account_id, eve_character_id, character_name }` with `actor_account_id = Some(account_id)` and `acting_as = None`.

The above emissions SHALL be ordered after `promote_if_no_main` so that any state needed for snapshot resolution is in place.

If any audit emission fails, the entire transaction (including the state change) SHALL be rolled back. This is the inherent behaviour of `record_in_tx` participating in the caller's transaction; the SSO callback service does not catch and ignore audit errors.

#### Scenario: First-character registration emits account_registered with non-null actor character
- **WHEN** a brand-new EVE character completes SSO and a new account is created
- **THEN** an `audit_log` row exists with `event_type = "account_registered"`, `actor_account_id = NULL`, `actor_character_id = <the signing-in EVE character ID>`, `actor_character_name = <the signing-in character name>`, and `details` containing `account_id`, `eve_character_id`, and `character_name`

#### Scenario: First account ever also emits server_admin_granted with bootstrap source
- **WHEN** the very first account is created via SSO (no prior `account` rows exist)
- **THEN** two `audit_log` rows exist for that transaction: `account_registered` and `server_admin_granted` with `details.source = "first_account_bootstrap"`

#### Scenario: Orphan-claim during login emits orphan_character_claimed
- **GIVEN** an `eve_character` row exists with `account_id IS NULL`
- **WHEN** that character completes SSO for the first time
- **THEN** an `audit_log` row exists with `event_type = "orphan_character_claimed"`, `actor_account_id = NULL`, `actor_character_id = <the EVE character ID>`, `actor_character_name = <the character name>`, and `details` containing `account_id`, `eve_character_id`, `character_name`

#### Scenario: Re-login of a soft-deleted account emits account_reactivated
- **GIVEN** an account with `status = 'soft_deleted'`
- **WHEN** one of its characters completes SSO and the account is reactivated
- **THEN** an `audit_log` row exists with `event_type = "account_reactivated"`, `actor_account_id = NULL`, `actor_character_id = <the logging-in character's EVE ID>`, `actor_character_name = <that character's name>`, and `details.account_id` matches the reactivated account

#### Scenario: Add-character flow emits character_added with the account's main as actor character
- **GIVEN** an authenticated session for an account whose main character is "Main Pilot"
- **WHEN** that account adds a second character via `/auth/characters/add` and SSO completes
- **THEN** an `audit_log` row exists with `event_type = "character_added"`, `actor_account_id = <the account ID>`, `actor_character_id = <Main Pilot's EVE ID>`, `actor_character_name = "Main Pilot"`, and `details` containing `eve_character_id` and `character_name` of the newly added character (not the main)

#### Scenario: Add-character flow claiming an orphan emits orphan_character_claimed with main actor
- **GIVEN** an authenticated session and an existing orphan `eve_character` row
- **WHEN** the account adds that orphan as a character via `/auth/characters/add` and SSO completes
- **THEN** an `audit_log` row exists with `event_type = "orphan_character_claimed"`, `actor_account_id = <the account ID>`, `actor_character_id = <the account's main EVE ID>`, `actor_character_name = <the main's name>`

#### Scenario: Audit emission failure rolls back the SSO callback transaction
- **GIVEN** a transient database failure that occurs during the audit emission step of the SSO callback transaction
- **WHEN** the transaction attempts to commit
- **THEN** the transaction is rolled back; no `eve_character` row is created or modified, no session is established, no audit row is written; the user-facing response is HTTP 5xx

### Requirement: SSO callback rejects blocked characters

The OAuth2 callback handler SHALL, after resolving the `eve_character_id` from the access-token JWT and **before** any account or character write, check whether that `eve_character_id` is present in `blocked_eve_character` (per the `server-administration` capability). If it is blocked, the callback SHALL reject the login: it SHALL NOT create or modify any `account` or `eve_character` row, SHALL NOT persist tokens, SHALL NOT create or update a session, and SHALL NOT set a session cookie. The browser SHALL be redirected to the `/blocked` information page (or an equivalent blocked response).

This check SHALL apply to **both** the login flow and the add-character flow, so that a blocked pilot can neither sign in as themselves nor be attached as an alt to an existing (even unblocked) account.

The rejection SHALL emit a `BlockedLoginRejected { eve_character_id }` audit event (per the `audit-log` capability) with `actor_account_id = NULL` (no account is authenticated) and the `eve_character_id` carried in `details`.

#### Scenario: Blocked character cannot log in
- **GIVEN** an `eve_character_id` present in `blocked_eve_character`
- **WHEN** that character completes the SSO flow at `/auth/callback`
- **THEN** no `account` or `eve_character` row is created or modified, no session is established, no session cookie is set, and the browser is redirected to `/blocked`; an `audit_log` row with `event_type = "blocked_login_rejected"` and `details.eve_character_id` equal to that id exists

#### Scenario: Blocked character cannot be added as an alt
- **GIVEN** an authenticated session for an unblocked account, and a blocked `eve_character_id`
- **WHEN** the account attempts to add that blocked character via the add-character flow and SSO completes
- **THEN** the blocked character is not attached to the account, no token is persisted for it, and a `blocked_login_rejected` audit row is written

#### Scenario: Block check precedes account creation
- **WHEN** a never-before-seen blocked `eve_character_id` completes SSO
- **THEN** the block is detected before any `account` row would be created, so no orphaned account results from the rejected login
