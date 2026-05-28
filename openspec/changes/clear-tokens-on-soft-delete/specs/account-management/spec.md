## MODIFIED Requirements

### Requirement: DELETE /api/v1/account soft-deletes the caller's account

`DELETE /api/v1/account` SHALL initiate soft-delete on the authenticated account by setting `account.status = 'soft_deleted'` and `account.delete_requested_at = now()`. It SHALL delete every row in the `session` table belonging to that account (so any other browser the user is logged in on is immediately logged out) and SHALL clear the caller's session cookie in the response. API keys belonging to the account SHALL NOT be deleted (a soft-deleted account that reactivates by re-login keeps its keys).

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

#### Scenario: Soft-delete is atomic with token clear
- **WHEN** the database fails partway through the soft-delete transaction
- **THEN** the transaction is rolled back: either both the `account.status` flip and the token-column clear take effect, or neither does

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
