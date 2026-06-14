## Purpose

HTTP endpoints under `/api/v1/` for reading the authenticated account and its characters (`GET /me`), promoting a character to main (`POST /characters/:id/set-main`), removing a character (`DELETE /characters/:id`), and soft-deleting the account (`DELETE /account`).
## Requirements

All `/api/*` request and response bodies in this capability conform to the `api-contract` spec (success envelope, error envelope, canonical error codes, RFC 3339 timestamps). Endpoint shapes below describe the contents of `data` for success and the contents of `error.details` (where applicable) for failure. All endpoints in this capability require authentication per the `api-authentication` capability (session cookie or `Authorization: Bearer erb_…`).

### Requirement: GET /api/v1/me returns the caller's account and characters

`GET /api/v1/me` SHALL return the authenticated account's identity and the full list of `eve_character` rows belonging to it. The response SHALL NOT include any raw token material (`encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, `scopes`). The response MAY include fields *derived* from those columns where the derivation does not reveal the underlying value — `token_status` (defined below) is such a derivation.

The `data` payload SHALL have the shape:

```json
{
  "account": {
    "id": "<uuid>",
    "status": "active" | "soft_deleted",
    "is_server_admin": true | false,
    "created_at": "<iso8601>"
  },
  "characters": [
    {
      "id": "<uuid>",
      "eve_character_id": <bigint>,
      "name": "<string>",
      "corporation_id": <bigint>,
      "corporation_name": "<string>",
      "alliance_id": <bigint> | null,
      "alliance_name": "<string> | null",
      "is_main": true | false,
      "portrait_url": "<string>",
      "token_status": "active" | "expired"
    }
  ]
}
```

`portrait_url` SHALL be the EVE image server URL of the form `https://images.evetech.net/characters/<eve_character_id>/portrait?size=128`. `corporation_name` and `alliance_name` SHALL be read directly from the corresponding columns on the `eve_character` row; `GET /api/v1/me` SHALL NOT make any ESI calls. Those columns are populated and refreshed by the SSO callback (per the `eve-sso-auth` capability) and by a future background job; the displayed name therefore reflects the value at the time of the most recent write, not necessarily live ESI state. Accepting that staleness is a deliberate trade — `GET /api/v1/me` is called on every authenticated frontend page load and MUST be cheap.

`token_status` SHALL be `"active"` when the row's `encrypted_refresh_token IS NOT NULL` and `"expired"` when `encrypted_refresh_token IS NULL`. It is a **string enum**, not a boolean; future scope-set or revocation states (e.g. `"missing_scopes"` once required-scope-set drift detection lands, or a non-`"active"` state set by a future refresh-on-demand flow when ESI returns `invalid_grant`) MAY be added without a breaking change. Neither `access_token_expires_at` nor `scopes` SHALL be returned.

**Why this rule, not the access-token expiry.** The access token's expiry (~20 minutes after login) is an implementation detail of the EVE SSO contract. Deriving `token_status` from it would flip every character to `"expired"` 20 minutes after every login even though the refresh token is still good, producing a noisy `re-auth` prompt the user does not need. The refresh token's usability is the real signal: while we hold one, we can transparently obtain a fresh access token; when we don't, the user must re-do SSO. Until a future change adds refresh-on-demand (which will NULL out `encrypted_refresh_token` on ESI `invalid_grant`), `token_status` will not surface refresh tokens that ESI has revoked server-side — the foundation change does not make ESI calls on the user's behalf, so a revoked refresh token has no user-visible consequence yet. This trade is documented in `design.md` Risks/Trade-offs.

#### Scenario: Authenticated caller fetches their account
- **WHEN** an authenticated caller `GET /api/v1/me`
- **THEN** the response is `200` with `data.account.id` equal to the caller's account ID and `data.characters` containing every `eve_character` row where `account_id` matches

#### Scenario: Response excludes raw token material
- **WHEN** `GET /api/v1/me` returns characters
- **THEN** no element of `data.characters` contains `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, or `scopes`

#### Scenario: token_status is active when a refresh token is held
- **WHEN** a character row has `encrypted_refresh_token IS NOT NULL`
- **THEN** that character's `token_status` is `"active"`, regardless of the value of `access_token_expires_at`

#### Scenario: token_status is expired when no refresh token is held
- **WHEN** a character row has `encrypted_refresh_token IS NULL` (e.g. an orphan that has never been signed-in-as)
- **THEN** that character's `token_status` is `"expired"`

#### Scenario: Exactly one character is flagged main
- **WHEN** the account has at least one linked character
- **THEN** exactly one element of `data.characters` has `is_main = true`

#### Scenario: New account with no main yet
- **WHEN** the account was just created and only one character is linked
- **THEN** that single character has `is_main = true` (the first linked character SHALL be promoted to main automatically)

#### Scenario: Unauthenticated caller is rejected
- **WHEN** a request to `GET /api/v1/me` has no session cookie and no valid bearer key
- **THEN** the response is HTTP 401 with the standard error envelope

#### Scenario: Handler makes no ESI calls
- **WHEN** `GET /api/v1/me` is served
- **THEN** the handler resolves `corporation_name` and `alliance_name` from `eve_character` columns only and makes zero outbound HTTPS requests to `esi.evetech.net` (or any ESI host) for the duration of the request

### Requirement: POST /api/v1/characters/:id/set-main promotes a character

`POST /api/v1/characters/:id/set-main` SHALL set `is_main = true` on the character with internal UUID `:id` and SHALL clear `is_main` on all other characters belonging to the same account, in a single transaction. The target character SHALL belong to the authenticated account.

On success the response SHALL be `200` with `data` equal to the updated character (same shape as one element of `GET /api/v1/me`'s `characters` array).

#### Scenario: Owner promotes a non-main character to main
- **WHEN** the caller owns characters A (main) and B (not main) and calls `POST /api/v1/characters/<B.id>/set-main`
- **THEN** B's `is_main` becomes `true`, A's `is_main` becomes `false`, the response is `200`, and the response `data.is_main` is `true`

#### Scenario: Promoting a character that is already main is a no-op success
- **WHEN** the caller calls `POST /api/v1/characters/<A.id>/set-main` and A is already the main
- **THEN** the response is `200` and A remains the main; no other rows are changed

#### Scenario: Non-owner cannot promote another account's character
- **WHEN** account X calls `POST /api/v1/characters/<id>/set-main` for a character belonging to account Y
- **THEN** the response is HTTP 404 (existence is not disclosed)

#### Scenario: Unknown character id
- **WHEN** `:id` does not match any `eve_character` row
- **THEN** the response is HTTP 404

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

### Requirement: POST /auth/characters/add is the canonical add-character entry point

The frontend's "add character" affordance on the characters page SHALL link to `GET /auth/characters/add` (defined in the `eve-sso-auth` capability). No new endpoint is introduced for this flow; this requirement exists only to document that the visible UI affordance has a defined backend route.

#### Scenario: Add-character link target
- **WHEN** the characters page renders the "+ add character" button
- **THEN** its `href` is `/auth/characters/add`

### Requirement: Account-management mutating endpoints emit audit events

The following endpoints SHALL emit audit events (per the `audit-log` capability) into the same transaction that performs the state change. Each emission SHALL use `actor_account_id = Some(<authenticated account>)` and `acting_as = None`, since all of these endpoints require an authenticated session or bearer key.

- `DELETE /api/v1/account` SHALL emit `AccountDeletionRequested { account_id }`. The emission SHALL occur inside the same transaction as the `account.status = 'soft_deleted'` transition and the character-token clearing required by the existing soft-delete requirement.
- `POST /api/v1/characters/:id/set-main` SHALL emit `CharacterSetMain { account_id, eve_character_id }` where `eve_character_id` is the EVE ID of the newly-promoted character. The emission SHALL occur inside the same transaction as the `is_main` flip.
- `DELETE /api/v1/characters/:id` SHALL emit `CharacterRemoved { account_id, eve_character_id }` where `eve_character_id` is the EVE ID of the character being removed. The emission SHALL occur inside the same transaction as the `DELETE FROM eve_character` statement.

If any audit emission fails, the entire transaction (including the state change) SHALL be rolled back. The endpoint SHALL NOT swallow audit errors.

For `CharacterSetMain`, the actor-character snapshot reflects the account's main *at the time of the emission*. Because the audit row is written inside the same transaction as (and conventionally *before*) the `is_main` flip commits, the snapshot SHALL be the *outgoing* main, not the incoming one. This preserves the property that the main-history of an account is reconstructible from the audit log: the `details.eve_character_id` of each `character_set_main` row identifies the *new* main, while the snapshot identifies the main that was being displaced.

#### Scenario: DELETE /api/v1/account writes an account_deletion_requested audit row
- **WHEN** an authenticated caller successfully soft-deletes their account via `DELETE /api/v1/account`
- **THEN** an `audit_log` row exists with `event_type = "account_deletion_requested"`, `actor_account_id = <the caller's account ID>`, `actor_character_id` / `actor_character_name` populated from that account's main, and `details = {}` (empty object — actor carries the account)

#### Scenario: POST /api/v1/characters/:id/set-main writes a character_set_main audit row
- **GIVEN** an account whose current main is character A, and a non-main character B owned by the same account
- **WHEN** the caller `POST /api/v1/characters/<B.id>/set-main`
- **THEN** an `audit_log` row exists with `event_type = "character_set_main"`, `actor_account_id = <the caller>`, `actor_character_id = A.eve_character_id` (the outgoing main, snapshotted before the flip), `actor_character_name = A.name`, and `details.eve_character_id = B.eve_character_id` (the incoming main)

#### Scenario: DELETE /api/v1/characters/:id writes a character_removed audit row
- **WHEN** an account removes one of its characters via `DELETE /api/v1/characters/:id` and the request succeeds (HTTP 204)
- **THEN** an `audit_log` row exists with `event_type = "character_removed"`, `actor_account_id = <the caller>`, and `details.eve_character_id` equal to the removed character's EVE ID

#### Scenario: Audit emission failure rolls back the state change
- **GIVEN** a transient database failure on the audit emission within `DELETE /api/v1/account`
- **WHEN** the transaction attempts to commit
- **THEN** the transaction is rolled back; `account.status` remains `'active'`; no `audit_log` row is written; the client sees an HTTP 5xx response

#### Scenario: Rejected requests do not write audit rows
- **WHEN** `DELETE /api/v1/characters/:id` is rejected with HTTP 409 (e.g. `cannot_remove_main` or `cannot_remove_last_character`)
- **THEN** no `audit_log` row is written for the rejected request

### Requirement: is_server_admin from /me gates the admin UI affordance

`GET /api/v1/me` already returns `data.account.is_server_admin` (per the existing `GET /api/v1/me` requirement). The frontend SHALL use that field to decide whether to surface the admin-navigation affordance and whether to attempt admin routes. This requirement introduces no behavioural change to `GET /api/v1/me`; it records that the existing field is the authority for the admin-UI gate, so the gate and the backend's `AdminAccount` extractor agree on a single source of truth.

#### Scenario: Admin field drives the affordance
- **WHEN** `GET /api/v1/me` returns `data.account.is_server_admin = true`
- **THEN** the frontend MAY surface the admin affordance; when it is `false`, the frontend SHALL NOT surface it

#### Scenario: /me itself is unchanged
- **WHEN** any caller fetches `GET /api/v1/me`
- **THEN** the response shape and fields are exactly as defined by the existing `GET /api/v1/me` requirement; this change adds no field and removes none

### Requirement: Account carries a denormalized last-known-main identity snapshot

Because an ERB account's human-readable identity is derived entirely from its characters, an account that loses all its characters would become unnameable and unfindable. To keep such an account identifiable, the `account` row SHALL carry a denormalized snapshot of its main character: `last_known_main_character_id` (BIGINT, nullable, holding the main's `eve_character_id`) and `last_known_main_character_name` (TEXT, nullable, holding the main's name).

These columns are a **display/identity snapshot, never a foreign key and never a join key**. `last_known_main_character_id` SHALL NOT reference `eve_character`; the snapshot must survive the referenced character row being detached or removed. The single source of truth for "which live character is main" remains the `is_main = TRUE` flag.

The snapshot SHALL be maintained in the same transaction as every change to which character is main:

- When `characters::set_main` promotes a character, it SHALL set the owning account's `last_known_main_*` to that character's `eve_character_id` and name.
- When `promote_if_no_main` promotes a character, it SHALL set the owning account's `last_known_main_*` to that character's `eve_character_id` and name. (Because `promote_if_no_main` returns only whether it promoted, the caller SHALL pass the promoted character's `eve_character_id` and name so the snapshot can be written.)

A null snapshot SHALL mean only "no main has ever been observed" (an account transiently before its first character).

#### Scenario: Setting a main updates the account snapshot
- **WHEN** a character is promoted to main via `set_main`
- **THEN** in the same transaction the owning account's `last_known_main_character_id` and `last_known_main_character_name` are set to that character's `eve_character_id` and name

#### Scenario: Auto-promotion updates the account snapshot
- **WHEN** `promote_if_no_main` promotes a character on an account that had no main
- **THEN** in the same transaction the account's `last_known_main_*` snapshot is set to that character's `eve_character_id` and name

#### Scenario: Snapshot survives the main character leaving the account
- **WHEN** the character recorded in `last_known_main_*` is detached or removed and no new main is promoted (the account is now empty)
- **THEN** the `last_known_main_character_id` and `last_known_main_character_name` columns retain the last-observed values, keeping the account nameable

### Requirement: An emptied account becomes orphaned, not deleted

The `account.status` domain SHALL include a terminal value `'orphaned'`, distinct from `'soft_deleted'`. `'soft_deleted'` means "owner-recoverable by logging back in"; `'orphaned'` means "unreachable — the account has zero characters, so no SSO login can ever resolve to it again." An orphaned account SHALL NOT be reactivated by the login self-heal path (which keys on resolving a character to the account, impossible with zero characters).

When the last character is detached from an account (e.g. by transfer detection in the `eve-sso-auth` capability), the same transaction SHALL set that account's `status = 'orphaned'`. The account row SHALL be retained, not deleted, so its `last_known_main_*` snapshot and any resources it owns remain visible to admins.

#### Scenario: Detaching the last character orphans the account
- **WHEN** an account's only character is detached
- **THEN** in the same transaction the account's `status` becomes `'orphaned'` and the row is retained with its `last_known_main_*` snapshot intact

#### Scenario: Orphaned status is distinct from soft-deleted
- **WHEN** an account is `'orphaned'`
- **THEN** its status is not `'soft_deleted'`, and the login self-heal/reactivation path does not restore it to `'active'` (no character can resolve to it)

#### Scenario: Orphaned account remains visible to admins
- **WHEN** an admin views accounts
- **THEN** an orphaned account is listed and is nameable via its `last_known_main_character_name`, and any maps or ACLs it owned remain visible

