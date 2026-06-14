## Purpose

EVE ESI OAuth2 authentication flow — `GET /auth/login`, `GET /auth/callback`, `POST /auth/logout`, `GET /auth/characters/add` — plus the Postgres-backed session store, sliding 7-day expiry, session-cookie JWT, ESI scope handling, and ESI discovery-document caching that support it.
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
The system SHALL handle `GET /auth/callback` by exchanging the authorization code for access and refresh tokens via the ESI token endpoint, parsing the JWT access token to extract `eve_character_id`, `name`, `scp` (the granted ESI scopes), and `owner` (the character owner hash), and fetching `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name` from ESI public-info endpoints. The corp and alliance **names** are persisted alongside the IDs on the `eve_character` row so that downstream reads (notably `GET /api/v1/me`) do not need to call ESI.

The `scp` claim in the EVE access-token JWT MAY be either a single string (when one scope was granted) or an array of strings (when multiple scopes were granted). Implementations SHALL accept both shapes and normalise to a `TEXT[]` for persistence in `eve_character.scopes`.

The `owner` claim SHALL be persisted as `eve_character.owner_hash` on every successful callback. The column is nullable; a null stored value means "not yet observed".

On every successful callback the backend SHALL set the resolved `account.last_login = now()` (the account-level freshness clock the daily sweep's 7-day idle waterfall reads).

On every successful callback, for the character being written/claimed, the backend SHALL set `token_status = valid` and store fresh tokens. Because a successful callback presents a current owner hash, this self-heals a character previously flagged `token_expired` or `owner_mismatch` whose hash now matches (see the `character-token-lifecycle` capability).

**Owner-hash transfer detection at bind time.** Before resolving the bind, the backend SHALL look up any existing `eve_character` row for the `eve_character_id` and compare the presented `owner` claim against that row's stored `owner_hash`. The presented hash differing from a **non-null** stored hash is CCP's canonical proof the character was transferred to a different EVE account. A character SHALL be treated as **transferred** when, and only when, the presented `owner` claim is present, the stored `owner_hash` is present (non-null), and the two values differ. When the presented hash is absent, the stored hash is null, or the two match, the character SHALL NOT be treated as transferred (the conservative path — detaching never occurs on absent or unprovable evidence). Transfer detection applies at BOTH the login and add-character call sites of the SSO completion, evaluated inside the SSO-completion transaction so it cannot race a concurrent claim or unlink.

When a character is detected as transferred and its existing row is bound to an account **other than** the bind destination, the backend SHALL, in the same transaction:

1. **Detach and rebind** the `eve_character` row to the destination account — the session's account in add-character mode, or a freshly-resolved/new account in login mode — overwriting tokens, `owner_hash`, `scopes`, public-info, setting `token_status = 'valid'`, and bumping `updated_at`. The bound-elsewhere rejection SHALL NOT apply to a transferred character.
2. **Run the seller-side fixup** on the prior (now-former) account: if it still has one or more characters but no longer has a `is_main = TRUE` character, re-promote one of its remaining characters to main (updating the account's `last_known_main_*` snapshot per the `account-management` capability); if it now has **zero** characters it is unreachable, so transition it to `status = 'orphaned'` (per the `account-management` capability) — the account row is kept, never deleted.
3. **Emit a character-transferred audit event** actored by the destination account, snapshotting the former account's id and `last_known_main_character_name` into `details` (self-contained, no read-time resolution).

On success the backend SHALL:

1. Look up `eve_character` by `eve_character_id`:
   - **If no row exists**: create a new `account` row (when not in add-character mode) or use the current session's `account_id` (add-character mode); insert a new `eve_character` row with `account_id` set, encrypted access and refresh tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, `token_status = 'valid'`, and the public-info fields.
   - **If an orphan row exists** (`account_id IS NULL`): claim it by setting `account_id` to the resolved account, writing the encrypted tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, `token_status = 'valid'`, and refreshing the public-info fields.
   - **If a row exists bound to the destination account**: overwrite `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, set `token_status = 'valid'`, refresh public-info fields, bump `updated_at`.
   - **If a row exists bound to a different account and the character is detected as transferred** (presented hash present, stored hash non-null and differing): detach from the former account and rebind to the destination per the transfer-detection rules above.
2. If the resolved account has no character flagged `is_main = TRUE` after the row is written/claimed, the same transaction SHALL set `is_main = TRUE` on the just-written character. There SHALL always be exactly one main per account that has any characters.
3. If the resolved `account.status` is `'soft_deleted'`, atomically reactivate it: `status = 'active'`, `delete_requested_at = NULL`.
4. Set `account.last_login = now()` for the resolved account.
5. Establish or update a session: insert (or update, in add-character mode) a row in the Postgres `session` table keyed by session ID, with `account_id` set to the resolved account, `created_at = now()` for new rows, `last_seen_at = now()`, and `expires_at = now() + interval '7 days'`. The row does NOT hold token material; tokens live only in `eve_character`. (The in-flight OAuth2 record's transient fields are consumed by the callback itself and are not persisted onto the session row.)
6. Set the session cookie (`httpOnly`, `SameSite=Lax`) carrying a signed HS256 JWT whose `exp` claim is 7 days out, and redirect to the validated `return_to` path stashed in the in-flight OAuth2 record, or to `/` when none was stashed.

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

#### Scenario: Owner hash and login time are recorded on callback
- **WHEN** the callback writes or claims a character
- **THEN** the presented `owner` claim is stored as `owner_hash`, the character's `token_status` is `valid`, and the resolved account's `last_login` is set to now

#### Scenario: Callback self-heals a previously flagged character
- **WHEN** a character previously flagged `token_expired` or `owner_mismatch` is authenticated via the callback and the presented owner hash matches the stored hash
- **THEN** its `token_status` returns to `valid` and fresh tokens are stored

#### Scenario: Login with a transferred character detaches it from the seller's account
- **WHEN** a user logs in with a character whose presented `owner` hash differs from the non-null `owner_hash` stored on its existing row, which is bound to another (seller's) account
- **THEN** the character is detached from the seller's account and bound to a freshly-resolved account for the logging-in user (not the seller's), the seller's account undergoes the seller-side fixup, and a character-transferred audit event is recorded — the user is NOT logged into the seller's account

#### Scenario: Add-character with a transferred character rebinds it to the session account
- **WHEN** a logged-in user completes the add-character flow with a character whose presented `owner` hash differs from the non-null `owner_hash` stored on its existing row, which is bound to another account
- **THEN** the character is detached from the other account and bound to the session's account (no `bound_elsewhere` rejection), the former account undergoes the seller-side fixup, and a character-transferred audit event is recorded

#### Scenario: Transfer that empties the seller account orphans it
- **WHEN** the detached character was the seller account's only character
- **THEN** in the same transaction the seller account transitions to `status = 'orphaned'` and its row (with its `last_known_main_*` snapshot) is retained, never deleted

#### Scenario: Transfer that leaves the seller account its main intact does not re-promote
- **WHEN** the detached character was NOT the seller account's main and the account retains its `is_main = TRUE` character
- **THEN** the seller account keeps its existing main and is not orphaned

#### Scenario: Transfer that strips the seller account's main re-promotes a remaining character
- **WHEN** the detached character was the seller account's main but the account retains other characters
- **THEN** in the same transaction one remaining character is promoted to `is_main = TRUE` and the seller account's `last_known_main_*` snapshot is updated accordingly

#### Scenario: Absent presented hash falls back to bound-elsewhere
- **WHEN** an add-character flow presents a character bound to another account but the presented `owner` claim is absent
- **THEN** no detach occurs and the existing `bound_elsewhere` conflict outcome applies (conservative — no transfer can be proven)

#### Scenario: Matching hash on the same account is a normal re-login
- **WHEN** a user logs in with a character whose presented `owner` hash matches the stored hash on its existing row bound to that same user's account
- **THEN** it is treated as a normal re-login / self-heal with no detach and no transfer event

#### Scenario: Session row holds no token material
- **WHEN** any row in the `session` table is inspected
- **THEN** the row holds only `session_id`, `account_id`, `created_at`, `last_seen_at`, and `expires_at`; it does NOT hold the access or refresh token

#### Scenario: Invalid or missing state parameter
- **WHEN** `/auth/callback` is called with a missing or mismatched `state` parameter
- **THEN** the backend responds with HTTP 400 and does not create a session

#### Scenario: Token exchange failure
- **WHEN** the ESI token endpoint returns an error for the provided code
- **THEN** the backend responds with HTTP 502 and does not create a session

### Requirement: OAuth state is bound to the initiating browser
`GET /auth/login` and `GET /auth/characters/add` SHALL set an `auth_state` cookie carrying the generated CSRF state value, with attributes `HttpOnly; SameSite=Lax; Secure; Path=/auth; Max-Age=900`. `GET /auth/callback` SHALL reject the request with HTTP 400 unless the `auth_state` cookie is present and equals the `state` query parameter, and SHALL perform this check before consuming the in-flight OAuth record. The callback SHALL clear the `auth_state` cookie on every outcome (success, blocked, and error).

#### Scenario: Login sets the state cookie
- **WHEN** a browser requests `GET /auth/login`
- **THEN** the 302 response carries a `Set-Cookie` header for `auth_state` whose value is the same `state` sent to EVE SSO, with `HttpOnly`, `SameSite=Lax`, `Secure`, `Path=/auth`, and `Max-Age=900`

#### Scenario: Callback without the state cookie is rejected
- **WHEN** a request to `GET /auth/callback?code=…&state=<valid-inflight-state>` arrives with no `auth_state` cookie
- **THEN** the backend responds 400, no token exchange is performed, and the in-flight record is not consumed

#### Scenario: Callback with a mismatching state cookie is rejected
- **WHEN** a request to `GET /auth/callback` arrives whose `auth_state` cookie differs from the `state` query parameter
- **THEN** the backend responds 400 and no token exchange is performed

#### Scenario: Successful callback clears the state cookie
- **WHEN** a callback completes successfully in the browser that initiated the login
- **THEN** the response clears the `auth_state` cookie (`Max-Age=0`) alongside setting the session cookie

### Requirement: In-flight OAuth records expire
In-flight OAuth records SHALL expire 15 minutes after creation. The callback SHALL treat an expired record as absent (HTTP 400). The in-flight store SHALL drop expired records opportunistically when new records are inserted and SHALL cap its size at 10 000 records; when the cap is reached and no expired records can be dropped, `GET /auth/login` SHALL refuse the new login attempt with an error response rather than evicting a live record.

#### Scenario: Expired in-flight record is rejected at the callback
- **WHEN** a callback arrives more than 15 minutes after `GET /auth/login` created its in-flight record
- **THEN** the backend responds 400 (invalid or missing state) and no session is created

#### Scenario: Store does not grow without bound
- **WHEN** `GET /auth/login` is requested repeatedly without any callback completing
- **THEN** records older than 15 minutes are dropped as new records are inserted, and the store never holds more than 10 000 records

### Requirement: Logout clears session
The system SHALL handle `POST /auth/logout` by deleting the session row from the Postgres `session` table and clearing the session cookie. The response SHALL redirect to `/`. The route SHALL NOT be registered for GET: a state-changing logout MUST NOT be reachable via top-level navigation (cross-site links, browser prefetch), so `GET /auth/logout` SHALL respond with HTTP 405.

#### Scenario: Authenticated user logs out
- **WHEN** a browser with a valid session cookie submits `POST /auth/logout`
- **THEN** the row in the `session` table for that session ID is deleted, the session cookie is cleared, and the browser is redirected to `/`

#### Scenario: Unauthenticated user requests logout
- **WHEN** a browser with no session cookie submits `POST /auth/logout`
- **THEN** the backend redirects to `/` without error

#### Scenario: GET logout is refused
- **WHEN** a browser requests `GET /auth/logout`
- **THEN** the backend responds 405 and the session is not deleted

### Requirement: Add-character links additional character to session
The system SHALL handle `GET /auth/characters/add` by redirecting to the EVE SSO authorization endpoint in add-character mode. On callback, the new character's tokens SHALL be added to the existing session rather than creating a new session. The endpoint SHALL also accept the OPTIONAL `?return_to=<path>` query parameter described under "Login redirects to EVE SSO"; the callback honours it on completion.

If the authenticated character is already bound to a **different** account (`eve_character.account_id` set and not equal to the session's account) AND the character is **not** detected as transferred (i.e. the presented `owner` hash is absent, the stored hash is null, or the hashes match — see "OAuth2 callback exchanges code and persists character"), the callback SHALL treat this as a conflict outcome, not a success:

- no write SHALL occur to the existing character row (no token overwrite, no public-info refresh, no `owner_hash` update);
- no `character_added` or `orphan_character_claimed` audit event SHALL be emitted; instead the rejected attempt SHALL be recorded as `character_add_rejected_bound_elsewhere` (see the `audit-log` capability);
- the session SHALL be preserved (the conflict concerns the character, not the caller);
- the browser SHALL be redirected to the `return_to` destination (default `/characters`) carrying an `add_conflict=bound_elsewhere` query flag, which the frontend renders as a dismissible localised notice.

A character that **is** detected as transferred (presented hash present, stored hash non-null and differing) SHALL NOT be rejected as bound-elsewhere; it is detached from the prior account and rebound to the session's account per the transfer-detection rules.

The bound-elsewhere check SHALL be evaluated inside the SSO-completion transaction so it cannot race a concurrent claim or unlink of the same character.

#### Scenario: Authenticated user adds a second character
- **WHEN** a browser with a valid session cookie requests `GET /auth/characters/add`
- **THEN** the backend redirects to EVE SSO; on successful callback, the new character is appended to the session's character list and the browser is redirected to `/`

#### Scenario: Unauthenticated user attempts add-character
- **WHEN** a browser with no session cookie requests `GET /auth/characters/add`
- **THEN** the backend responds with HTTP 401

#### Scenario: Adding a non-transferred character bound to another account is refused
- **WHEN** account B's session completes the add-character flow as a character already bound to account A whose presented `owner` hash matches account A's stored hash (or no hash can be compared)
- **THEN** account A's `eve_character` row is unchanged (tokens, `owner_hash`, public-info, `account_id` all untouched), no character is added to account B, and the browser is redirected with `add_conflict=bound_elsewhere`

#### Scenario: Adding a transferred character bound to another account succeeds
- **WHEN** account B's session completes the add-character flow as a character bound to account A whose presented `owner` hash differs from account A's non-null stored hash
- **THEN** the character is detached from account A and bound to account B, account A undergoes the seller-side fixup, a character-transferred audit event is recorded, and no `add_conflict=bound_elsewhere` flag is returned

#### Scenario: The rejected attempt is audited truthfully
- **WHEN** the bound-elsewhere conflict occurs
- **THEN** a `character_add_rejected_bound_elsewhere` audit row is written with the session account as actor and the character as target, and no `character_added` row exists for the attempt

#### Scenario: The conflict notice is shown and dismissible
- **WHEN** the browser lands on the characters page with `add_conflict=bound_elsewhere`
- **THEN** a localised notice explains the character is already linked to another account, and the flag is removed from the URL after rendering so a reload does not re-show it

### Requirement: Session cookie does not expose token material
The session cookie SHALL contain only an encrypted, signed session ID — no access tokens, refresh tokens, character IDs, or other EVE credential material. The cookie SHALL be `httpOnly`, `SameSite=Lax`, and `Secure`. The `Secure` attribute SHALL be present on every path that sets or clears the session cookie: the SSO callback's initial `Set-Cookie`, the per-request sliding refresh, and the clear on logout and account deletion.

#### Scenario: Session cookie inspection reveals no token data
- **WHEN** a session is established after login
- **THEN** the `Set-Cookie` header value contains only an opaque session ID; access and refresh tokens are absent from all response headers and cookie values

#### Scenario: Session cookie carries Secure on set, refresh, and clear
- **WHEN** the session cookie is set at login, re-issued by the sliding refresh on an authenticated request, or cleared at logout
- **THEN** each `Set-Cookie` header includes the `Secure` attribute

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

