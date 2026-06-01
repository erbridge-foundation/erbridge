## MODIFIED Requirements

### Requirement: OAuth2 callback exchanges code and persists character
The system SHALL handle `GET /auth/callback` by exchanging the authorization code for access and refresh tokens via the ESI token endpoint, parsing the JWT access token to extract `eve_character_id`, `name`, `scp` (the granted ESI scopes), and `owner` (the character owner hash), and fetching `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name` from ESI public-info endpoints. The corp and alliance **names** are persisted alongside the IDs on the `eve_character` row so that downstream reads (notably `GET /api/v1/me`) do not need to call ESI.

The `scp` claim in the EVE access-token JWT MAY be either a single string (when one scope was granted) or an array of strings (when multiple scopes were granted). Implementations SHALL accept both shapes and normalise to a `TEXT[]` for persistence in `eve_character.scopes`.

The `owner` claim SHALL be persisted as `eve_character.owner_hash` on every successful callback. The column is nullable; a null stored value means "not yet observed".

On every successful callback the backend SHALL set the resolved `account.last_login = now()` (the account-level freshness clock the daily sweep's 7-day idle waterfall reads).

On every successful callback, for the character being written/claimed, the backend SHALL set `token_status = valid` and store fresh tokens. Because a successful callback presents a current owner hash, this self-heals a character previously flagged `token_expired` or `owner_mismatch` whose hash now matches (see the `character-token-lifecycle` capability). Detection of an owner-hash *change* is performed by the daily refresh sweep, not by the callback.

On success the backend SHALL:

1. Look up `eve_character` by `eve_character_id`:
   - **If no row exists**: create a new `account` row (when not in add-character mode) or use the current session's `account_id` (add-character mode); insert a new `eve_character` row with `account_id` set, encrypted access and refresh tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, `token_status = 'valid'`, and the public-info fields.
   - **If an orphan row exists** (`account_id IS NULL`): claim it by setting `account_id` to the resolved account, writing the encrypted tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, `token_status = 'valid'`, and refreshing the public-info fields.
   - **If a row exists with `account_id` set**: overwrite `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, set `token_status = 'valid'`, refresh public-info fields, bump `updated_at`. The row's `account_id` is the session's account.
2. If the resolved account has no character flagged `is_main = TRUE` after the row is written/claimed, the same transaction SHALL set `is_main = TRUE` on the just-written character. There SHALL always be exactly one main per account that has any characters.
3. If the resolved `account.status` is `'soft_deleted'`, atomically reactivate it: `status = 'active'`, `delete_requested_at = NULL`.
4. Set `account.last_login = now()` for the resolved account.
5. Establish or update a session: insert (or update, in add-character mode) a row in the Postgres `session` table keyed by session ID, with `account_id` set to the resolved account, `csrf_state` and `add_character_mode` carried from the in-flight OAuth2 record, `created_at = now()` for new rows, `last_seen_at = now()`, and `expires_at = now() + interval '7 days'`. The row does NOT hold token material; tokens live only in `eve_character`.
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

#### Scenario: Owner hash and login time are recorded on callback
- **WHEN** the callback writes or claims a character
- **THEN** the presented `owner` claim is stored as `owner_hash`, the character's `token_status` is `valid`, and the resolved account's `last_login` is set to now

#### Scenario: Callback self-heals a previously flagged character
- **WHEN** a character previously flagged `token_expired` or `owner_mismatch` is authenticated via the callback and the presented owner hash matches the stored hash
- **THEN** its `token_status` returns to `valid` and fresh tokens are stored

#### Scenario: Session row holds no token material
- **WHEN** any row in the `session` table is inspected
- **THEN** the row holds only `session_id`, `account_id`, `csrf_state`, `add_character_mode`, `created_at`, `last_seen_at`, and `expires_at`; it does NOT hold the access or refresh token

#### Scenario: Invalid or missing state parameter
- **WHEN** `/auth/callback` is called with a missing or mismatched `state` parameter
- **THEN** the backend responds with HTTP 400 and does not create a session

#### Scenario: Token exchange failure
- **WHEN** the ESI token endpoint returns an error for the provided code
- **THEN** the backend responds with HTTP 502 and does not create a session
