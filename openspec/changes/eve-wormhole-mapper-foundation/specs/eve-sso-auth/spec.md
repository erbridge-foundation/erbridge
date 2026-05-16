## ADDED Requirements

### Requirement: Login redirects to EVE SSO
The system SHALL redirect the browser to the EVE ESI authorization endpoint when `GET /auth/login` is requested. The authorization URL SHALL be derived from the ESI discovery document fetched at startup, never hardcoded. The redirect SHALL include all required OAuth2 parameters: `response_type=code`, `client_id`, `redirect_uri` (`{APP_URL}/auth/callback`), `scope` (the full required scope list), and a `state` parameter for CSRF protection.

#### Scenario: Unauthenticated user visits login
- **WHEN** a browser requests `GET /auth/login`
- **THEN** the backend responds with HTTP 302 redirecting to the EVE SSO authorization URL with all required query parameters

#### Scenario: Authorization URL is not hardcoded
- **WHEN** the backend starts up
- **THEN** it fetches `https://login.eveonline.com/.well-known/oauth-authorization-server` and uses `authorization_endpoint` from the response for all login redirects

### Requirement: OAuth2 callback exchanges code and persists character
The system SHALL handle `GET /auth/callback` by exchanging the authorization code for access and refresh tokens via the ESI token endpoint, parsing the JWT access token to extract `eve_character_id` and `name`, and fetching `corporation_id` / `alliance_id` from ESI public-info endpoints.

On success the backend SHALL:

1. Look up `eve_character` by `eve_character_id`:
   - **If no row exists**: create a new `account` row (when not in add-character mode) or use the current session's `account_id` (add-character mode); insert a new `eve_character` row with `account_id` set, encrypted access and refresh tokens, `esi_token_expires_at`, `esi_client_id`, and the public-info fields.
   - **If an orphan row exists** (`account_id IS NULL`): claim it by setting `account_id` to the resolved account, writing the encrypted tokens, `esi_token_expires_at`, `esi_client_id`, and refreshing the public-info fields.
   - **If a row exists with `account_id` set**: overwrite `encrypted_access_token`, `encrypted_refresh_token`, `esi_token_expires_at`, `esi_client_id`, refresh public-info fields, bump `updated_at`. The row's `account_id` is the session's account.
2. If the resolved `account.status` is `'soft_deleted'`, atomically reactivate it: `status = 'active'`, `delete_requested_at = NULL`.
3. Establish or update a session: an in-memory entry keyed by session ID, pointing to the resolved `account_id`. The session entry does NOT hold token material; tokens live only in Postgres.
4. Set the session cookie (`httpOnly`, `SameSite=Lax`) and redirect to `/`.

#### Scenario: Valid callback creates session and redirects home
- **WHEN** EVE SSO redirects to `/auth/callback` with a valid `code` and matching `state`
- **THEN** the backend exchanges the code for tokens, persists them encrypted in `eve_character`, creates an in-memory session pointing to the resolved account, sets a session cookie, and redirects the browser to `/`

#### Scenario: Orphan character is claimed on first login
- **WHEN** the callback resolves an `eve_character_id` that exists with `account_id = NULL`
- **THEN** the existing row's `account_id` is set to the (possibly newly-created) account; no duplicate row is created

#### Scenario: Login reactivates a soft-deleted account
- **WHEN** the callback resolves to an account with `status = 'soft_deleted'`
- **THEN** the same transaction that writes the tokens also sets `status = 'active'` and `delete_requested_at = NULL`

#### Scenario: Session entry holds no token material
- **WHEN** any session is inspected in the in-memory session store
- **THEN** the entry holds only `account_id` and CSRF / routing state; it does NOT hold the access or refresh token

#### Scenario: Invalid or missing state parameter
- **WHEN** `/auth/callback` is called with a missing or mismatched `state` parameter
- **THEN** the backend responds with HTTP 400 and does not create a session

#### Scenario: Token exchange failure
- **WHEN** the ESI token endpoint returns an error for the provided code
- **THEN** the backend responds with HTTP 502 and does not create a session

### Requirement: Logout clears session
The system SHALL handle `GET /auth/logout` by removing the session from the server-side store and clearing the session cookie. The response SHALL redirect to `/`.

#### Scenario: Authenticated user logs out
- **WHEN** a browser with a valid session cookie requests `GET /auth/logout`
- **THEN** the session is removed from the store, the session cookie is cleared, and the browser is redirected to `/`

#### Scenario: Unauthenticated user requests logout
- **WHEN** a browser with no session cookie requests `GET /auth/logout`
- **THEN** the backend redirects to `/` without error

### Requirement: Add-character links additional character to session
The system SHALL handle `GET /auth/characters/add` by redirecting to the EVE SSO authorization endpoint in add-character mode. On callback, the new character's tokens SHALL be added to the existing session rather than creating a new session.

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
