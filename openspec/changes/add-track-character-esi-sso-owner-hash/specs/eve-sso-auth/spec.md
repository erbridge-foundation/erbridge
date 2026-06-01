## MODIFIED Requirements

### Requirement: OAuth2 callback exchanges code and persists character
The system SHALL handle `GET /auth/callback` by exchanging the authorization code for access and refresh tokens via the ESI token endpoint, parsing the JWT access token to extract `eve_character_id`, `name`, `scp` (the granted ESI scopes), and `owner` (the character owner hash), and fetching `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name` from ESI public-info endpoints. The corp and alliance **names** are persisted alongside the IDs on the `eve_character` row so that downstream reads (notably `GET /api/v1/me`) do not need to call ESI.

The `scp` claim in the EVE access-token JWT MAY be either a single string (when one scope was granted) or an array of strings (when multiple scopes were granted). Implementations SHALL accept both shapes and normalise to a `TEXT[]` for persistence in `eve_character.scopes`.

The `owner` claim SHALL be persisted as `eve_character.owner_hash` on every successful callback. The column is nullable; a null stored value means "not yet observed" and is never treated as a transfer.

On success the backend SHALL:

1. Look up `eve_character` by `eve_character_id`:
   - **If no row exists**: create a new `account` row (when not in add-character mode) or use the current session's `account_id` (add-character mode); insert a new `eve_character` row with `account_id` set, encrypted access and refresh tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, and the public-info fields.
   - **If an orphan row exists** (`account_id IS NULL`): claim it by setting `account_id` to the resolved account, writing the encrypted tokens, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, and refreshing the public-info fields.
   - **If a row exists with `account_id` set**:
     - **Transfer check**: if the stored `owner_hash` is non-null and differs from the presented `owner` claim, the character has been transferred. In the same transaction, before re-linking: (a) clear the credential columns (`encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `scopes`) on **all** of the previous owner's `eve_character` rows; (b) delete **all** of the previous owner's `session` rows; (c) emit a character-transfer audit event recording `eve_character_id`, the previous `account_id`, and the resolved (new) `account_id`; (d) set the row's `account_id = NULL` and `is_main = FALSE`. Processing then continues as for an orphan row (claim by the resolved account), storing the new `owner_hash`.
     - **Otherwise** (no transfer): overwrite `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, `scopes`, `owner_hash`, refresh public-info fields, bump `updated_at`. The row's `account_id` is the session's account.
2. If the resolved account has no character flagged `is_main = TRUE` after the row is written/claimed (this is true for any account that just gained its first character, including via orphan-claim), the same transaction SHALL set `is_main = TRUE` on the just-written character. There SHALL always be exactly one main per account that has any characters.
3. If the resolved `account.status` is `'soft_deleted'`, atomically reactivate it: `status = 'active'`, `delete_requested_at = NULL`.
4. Establish or update a session: insert (or update, in add-character mode) a row in the Postgres `session` table keyed by session ID, with `account_id` set to the resolved account, `csrf_state` and `add_character_mode` carried from the in-flight OAuth2 record, `created_at = now()` for new rows, `last_seen_at = now()`, and `expires_at = now() + interval '7 days'`. The row does NOT hold token material; tokens live only in `eve_character`. The session being established is for the authenticating (resolved) account; a transfer in step 1 only clears the previous owner's sessions, never the resolved account's.
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

#### Scenario: Owner hash is recorded on first observation
- **WHEN** the callback writes or claims a character whose stored `owner_hash` is null (first link, orphan-claim, or a legacy row predating the column)
- **THEN** the presented `owner` claim is stored as `owner_hash` and no transfer enforcement runs

#### Scenario: Changed owner hash detaches the character and severs the previous owner
- **WHEN** the callback resolves a character with `account_id` set to account A, a non-null stored `owner_hash`, and a presented `owner` claim that differs, while authenticating as account B
- **THEN** the same transaction clears credential columns on all of A's characters, deletes all of A's sessions, emits a character-transfer audit event (`eve_character_id`, old = A, new = B), detaches the row to `account_id = NULL` / `is_main = FALSE`, then claims it for B with fresh tokens and the new `owner_hash`; account B's session is created normally

#### Scenario: Session row holds no token material
- **WHEN** any row in the `session` table is inspected
- **THEN** the row holds only `session_id`, `account_id`, `csrf_state`, `add_character_mode`, `created_at`, `last_seen_at`, and `expires_at`; it does NOT hold the access or refresh token

#### Scenario: Invalid or missing state parameter
- **WHEN** `/auth/callback` is called with a missing or mismatched `state` parameter
- **THEN** the backend responds with HTTP 400 and does not create a session

#### Scenario: Token exchange failure
- **WHEN** the ESI token endpoint returns an error for the provided code
- **THEN** the backend responds with HTTP 502 and does not create a session
