# eve-sso-auth — delta for surface-add-character-conflict

## MODIFIED Requirements

### Requirement: Add-character links additional character to session
The system SHALL handle `GET /auth/characters/add` by redirecting to the EVE SSO authorization endpoint in add-character mode. On callback, the new character's tokens SHALL be added to the existing session rather than creating a new session. The endpoint SHALL also accept the OPTIONAL `?return_to=<path>` query parameter described under "Login redirects to EVE SSO"; the callback honours it on completion.

If the authenticated character is already bound to a **different** account (`eve_character.account_id` set and not equal to the session's account), the callback SHALL treat this as a conflict outcome, not a success:

- no write SHALL occur to the existing character row (no token overwrite, no public-info refresh, no `owner_hash` update);
- no `character_added` or `orphan_character_claimed` audit event SHALL be emitted; instead the rejected attempt SHALL be recorded as `character_add_rejected_bound_elsewhere` (see the `audit-log` capability);
- the session SHALL be preserved (the conflict concerns the character, not the caller);
- the browser SHALL be redirected to the `return_to` destination (default `/characters`) carrying an `add_conflict=bound_elsewhere` query flag, which the frontend renders as a dismissible localised notice.

The bound-elsewhere check SHALL be evaluated inside the SSO-completion transaction so it cannot race a concurrent claim or unlink of the same character.

#### Scenario: Authenticated user adds a second character
- **WHEN** a browser with a valid session cookie requests `GET /auth/characters/add`
- **THEN** the backend redirects to EVE SSO; on successful callback, the new character is appended to the session's character list and the browser is redirected to `/`

#### Scenario: Unauthenticated user attempts add-character
- **WHEN** a browser with no session cookie requests `GET /auth/characters/add`
- **THEN** the backend responds with HTTP 401

#### Scenario: Adding a character bound to another account is refused
- **WHEN** account B's session completes the add-character flow as a character already bound to account A
- **THEN** account A's `eve_character` row is unchanged (tokens, `owner_hash`, public-info, `account_id` all untouched), no character is added to account B, and the browser is redirected with `add_conflict=bound_elsewhere`

#### Scenario: The rejected attempt is audited truthfully
- **WHEN** the bound-elsewhere conflict occurs
- **THEN** a `character_add_rejected_bound_elsewhere` audit row is written with the session account as actor and the character as target, and no `character_added` row exists for the attempt

#### Scenario: The conflict notice is shown and dismissible
- **WHEN** the browser lands on the characters page with `add_conflict=bound_elsewhere`
- **THEN** a localised notice explains the character is already linked to another account, and the flag is removed from the URL after rendering so a reload does not re-show it
