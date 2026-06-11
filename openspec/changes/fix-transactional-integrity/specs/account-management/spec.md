# account-management — delta for fix-transactional-integrity

## MODIFIED Requirements

### Requirement: DELETE /api/v1/account soft-deletes the caller's account

`DELETE /api/v1/account` SHALL initiate soft-delete on the authenticated account by setting `account.status = 'soft_deleted'` and `account.delete_requested_at = now()`. It SHALL delete every row in the `session` table belonging to that account (so any other browser the user is logged in on is immediately logged out) and SHALL clear the caller's session cookie in the response. API keys belonging to the account SHALL NOT be deleted (a soft-deleted account that reactivates by re-login keeps its keys).

The status flip, the EVE-credential clear, and the session deletion SHALL all occur in the same transaction: a soft-deleted account MUST NOT retain a usable session under any partial-failure ordering, because cookie-path authentication enforces soft-delete solely through session absence.

If the account is a server admin, the last-admin guard (refuse with HTTP 409 `cannot_remove_last_server_admin` when no other active admin would remain) SHALL be evaluated inside the same transaction as the status flip, such that two concurrent deletion requests from the final two admin accounts cannot both succeed.

Whenever an `account` row transitions to `status = 'soft_deleted'` — via this endpoint or any future path that performs the same transition — every `eve_character` row owned by that account SHALL, in the same transaction as the status change, have its EVE-credential columns cleared:

- `encrypted_access_token` SHALL be set to `NULL`
- `encrypted_refresh_token` SHALL be set to `NULL`
- `access_token_expires_at` SHALL be set to `NULL`
- `scopes` SHALL be set to the empty array

The `eve_character` rows themselves SHALL NOT be deleted (the rows must remain so reactivation can find and re-bind them). Columns that identify the character (name, corporation, alliance, `eve_character_id`, `account_id`, `is_main`) SHALL NOT be modified.

This clears the service's own copy of the EVE credentials. It does NOT call EVE's SSO token-revocation endpoint and SHALL NOT be relied upon as a guarantee that the app's authorisation at EVE SSO has been withdrawn; users concerned about credential compromise are expected to revoke authorisation at EVE SSO themselves.

The response SHALL be HTTP 204 with no body and EXACTLY ONE `Set-Cookie` header that clears the session cookie (name `session`, empty value, `Max-Age=0`). The server SHALL NOT, on the same response, emit any additional `Set-Cookie` header that refreshes the session — implementations using a session-refresh middleware must suppress it for this response so the browser is reliably logged out.

A subsequent SSO login as any of the account's characters SHALL reactivate the account per the `eve-sso-auth` capability (status returns to `'active'`, `delete_requested_at` cleared). Only the character that completes the SSO login receives fresh tokens via the existing upsert path; other linked characters remain with `encrypted_refresh_token = NULL` and report `token_status = "expired"` until each is individually re-authorised through the account page.

#### Scenario: Owner soft-deletes their own account
- **WHEN** the authenticated caller calls `DELETE /api/v1/account`
- **THEN** their `account.status` becomes `'soft_deleted'`, `delete_requested_at` is set to `now()`, all of their `session` rows are deleted, the response is HTTP 204, and the response contains exactly one `Set-Cookie` header that clears the session cookie (`Max-Age=0`) — no refresh cookie is emitted alongside it

#### Scenario: Soft-delete clears EVE-credential columns on every linked character
- **WHEN** an account with one or more linked `eve_character` rows is soft-deleted
- **THEN** in the same transaction as the `account.status` flip, every owned `eve_character` row has `encrypted_access_token = NULL`, `encrypted_refresh_token = NULL`, `access_token_expires_at = NULL`, and `scopes = '{}'`, while the row itself and its identity columns (name, corporation, alliance, `eve_character_id`, `account_id`, `is_main`) are preserved

#### Scenario: Soft-delete is atomic with token clear and session deletion
- **WHEN** the database fails partway through the soft-delete transaction
- **THEN** the transaction is rolled back: the `account.status` flip, the token-column clear, and the `session`-row deletion either all take effect or none do — at no point does a soft-deleted account retain a live session row

#### Scenario: Concurrent deletion by the last two admins cannot remove both
- **WHEN** the only two active server-admin accounts each call `DELETE /api/v1/account` concurrently
- **THEN** at most one request succeeds; the other receives HTTP 409 `cannot_remove_last_server_admin`, and at least one active server admin remains

#### Scenario: Account row remains queryable while soft-deleted
- **WHEN** an account is soft-deleted
- **THEN** the row still exists in `account` and its `eve_character` rows still exist (with EVE-credential columns cleared per the scenario above)

#### Scenario: Re-login reactivates a soft-deleted account and refreshes the logging-in character only
- **WHEN** a soft-deleted account's pilot completes SSO login as one of the linked characters
- **THEN** per the `eve-sso-auth` capability, `status` is set back to `'active'` and `delete_requested_at` is cleared in the same transaction that writes the ESI tokens for the character that just logged in; that character's `token_status` becomes `"active"`, and every other linked character continues to report `token_status = "expired"` until re-authorised individually

#### Scenario: Soft-delete does not revoke EVE-side authorisation
- **WHEN** an account is soft-deleted
- **THEN** the service makes no outbound call to EVE SSO to revoke the app's authorisation, and the spec makes no guarantee that previously-issued refresh tokens are unusable from EVE's perspective; revoking at EVE SSO is the user's responsibility

#### Scenario: Bearer token continues to work until soft-delete reactivation path is considered
- **WHEN** a request presents an API key whose owning account has just been soft-deleted
- **THEN** the request is rejected with HTTP 401 and `error.code = "account_soft_deleted"`; the key row is not deleted

### Requirement: DELETE /api/v1/characters/:id unlinks a character

`DELETE /api/v1/characters/:id` SHALL hard-delete the `eve_character` row identified by `:id` if and only if it belongs to the authenticated account. On success the response SHALL be HTTP 204 with no body.

The backend SHALL refuse to delete the account's only character with HTTP 409 — removing the final character is not permitted because it would leave the account without an identity. The user must delete the account itself (per the `DELETE /api/v1/account` requirement below) to remove the final character.

The backend SHALL refuse to delete the character flagged `is_main = true` while at least one other character exists, with HTTP 409. The caller must promote another character to main first.

Both guards SHALL be evaluated inside the same transaction as the delete, against the state that includes the transaction's own pending write, such that concurrent requests cannot jointly violate either invariant (e.g. two parallel deletes on a two-character account removing both rows).

#### Scenario: Owner removes a non-main character with siblings
- **WHEN** the caller owns characters A (main) and B (not main) and calls `DELETE /api/v1/characters/<B.id>`
- **THEN** B's row is hard-deleted, the response is HTTP 204, and A remains the main

#### Scenario: Removing the main character is rejected when other characters exist
- **WHEN** the caller owns characters A (main) and B (not main) and calls `DELETE /api/v1/characters/<A.id>`
- **THEN** the response is HTTP 409 with `error.code = "cannot_remove_main"`; no row is deleted

#### Scenario: Removing the only character is rejected
- **WHEN** the caller's account has exactly one character and calls `DELETE /api/v1/characters/<id>`
- **THEN** the response is HTTP 409 with `error.code = "cannot_remove_last_character"`; the row is not deleted

#### Scenario: Concurrent deletes cannot empty an account
- **WHEN** the caller owns exactly characters A (main) and B (not main) and two `DELETE /api/v1/characters/<B.id>`-style requests for the deletable set race
- **THEN** at most one delete succeeds and the account always retains at least one character, with the survivor flagged `is_main = true`

#### Scenario: Non-owner cannot remove another account's character
- **WHEN** account X calls `DELETE /api/v1/characters/<id>` for a character belonging to account Y
- **THEN** the response is HTTP 404; no row is deleted

#### Scenario: Unknown character id
- **WHEN** `:id` does not match any `eve_character` row
- **THEN** the response is HTTP 404
