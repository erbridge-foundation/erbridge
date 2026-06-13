## MODIFIED Requirements

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
