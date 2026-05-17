## ADDED Requirements

All `/api/*` request and response bodies in this capability conform to the `api-contract` spec (success envelope, error envelope, canonical error codes, RFC 3339 timestamps). Endpoint shapes below describe the contents of `data` for success and the contents of `error.details` (where applicable) for failure. All endpoints in this capability require authentication per the `api-authentication` capability (session cookie or `Authorization: Bearer erb_…`).

### Requirement: GET /api/v1/me returns the caller's account and characters

`GET /api/v1/me` SHALL return the authenticated account's identity and the full list of `eve_character` rows belonging to it. The response SHALL NOT include any token material (`encrypted_access_token`, `encrypted_refresh_token`, `esi_token_expires_at`, `esi_client_id`).

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
      "portrait_url": "<string>"
    }
  ]
}
```

`portrait_url` SHALL be the EVE image server URL of the form `https://images.evetech.net/characters/<eve_character_id>/portrait?size=128`. `corporation_name` and `alliance_name` SHALL be fetched from ESI public-info endpoints; the backend MAY cache these lookups in process memory for the request lifetime.

#### Scenario: Authenticated caller fetches their account
- **WHEN** an authenticated caller `GET /api/v1/me`
- **THEN** the response is `200` with `data.account.id` equal to the caller's account ID and `data.characters` containing every `eve_character` row where `account_id` matches

#### Scenario: Response excludes token material
- **WHEN** `GET /api/v1/me` returns characters
- **THEN** no element of `data.characters` contains `encrypted_access_token`, `encrypted_refresh_token`, `esi_token_expires_at`, or `esi_client_id`

#### Scenario: Exactly one character is flagged main
- **WHEN** the account has at least one linked character
- **THEN** exactly one element of `data.characters` has `is_main = true`

#### Scenario: New account with no main yet
- **WHEN** the account was just created and only one character is linked
- **THEN** that single character has `is_main = true` (the first linked character SHALL be promoted to main automatically)

#### Scenario: Unauthenticated caller is rejected
- **WHEN** a request to `GET /api/v1/me` has no session cookie and no valid bearer key
- **THEN** the response is HTTP 401 with the standard error envelope

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

#### Scenario: Owner removes a non-main character with siblings
- **WHEN** the caller owns characters A (main) and B (not main) and calls `DELETE /api/v1/characters/<B.id>`
- **THEN** B's row is hard-deleted, the response is HTTP 204, and A remains the main

#### Scenario: Removing the main character is rejected when other characters exist
- **WHEN** the caller owns characters A (main) and B (not main) and calls `DELETE /api/v1/characters/<A.id>`
- **THEN** the response is HTTP 409 with `error.code = "cannot_remove_main"`; no row is deleted

#### Scenario: Removing the only character is rejected
- **WHEN** the caller's account has exactly one character and calls `DELETE /api/v1/characters/<id>`
- **THEN** the response is HTTP 409 with `error.code = "cannot_remove_last_character"`; the row is not deleted

#### Scenario: Non-owner cannot remove another account's character
- **WHEN** account X calls `DELETE /api/v1/characters/<id>` for a character belonging to account Y
- **THEN** the response is HTTP 404; no row is deleted

#### Scenario: Unknown character id
- **WHEN** `:id` does not match any `eve_character` row
- **THEN** the response is HTTP 404

### Requirement: DELETE /api/v1/account soft-deletes the caller's account

`DELETE /api/v1/account` SHALL initiate soft-delete on the authenticated account by setting `account.status = 'soft_deleted'` and `account.delete_requested_at = now()`. It SHALL invalidate every in-memory session belonging to that account and SHALL clear the caller's session cookie in the response. Character rows SHALL NOT be modified. API keys belonging to the account SHALL NOT be deleted (a soft-deleted account that reactivates by re-login keeps its keys).

The response SHALL be HTTP 204 with no body and a `Set-Cookie` header that clears the session cookie.

A subsequent SSO login as any of the account's characters SHALL reactivate the account per the `eve-sso-auth` capability (status returns to `'active'`, `delete_requested_at` cleared) — this is the documented recovery path, not a separate endpoint.

#### Scenario: Owner soft-deletes their own account
- **WHEN** the authenticated caller calls `DELETE /api/v1/account`
- **THEN** their `account.status` becomes `'soft_deleted'`, `delete_requested_at` is set to `now()`, all of their in-memory sessions are dropped, the response is HTTP 204, and the response clears the session cookie

#### Scenario: Account row remains queryable while soft-deleted
- **WHEN** an account is soft-deleted
- **THEN** the row still exists in `account` and its `eve_character` rows are unchanged

#### Scenario: Re-login reactivates a soft-deleted account
- **WHEN** the soft-deleted account's pilot completes SSO login
- **THEN** per the `eve-sso-auth` capability, `status` is set back to `'active'` and `delete_requested_at` is cleared in the same transaction that writes the ESI tokens

#### Scenario: Bearer token continues to work until soft-delete reactivation path is considered
- **WHEN** a request presents an API key whose owning account has just been soft-deleted
- **THEN** the request is rejected with HTTP 401 and `error.code = "account_soft_deleted"`; the key row is not deleted

### Requirement: POST /auth/characters/add is the canonical add-character entry point

The frontend's "add character" affordance on the characters page SHALL link to `GET /auth/characters/add` (defined in the `eve-sso-auth` capability). No new endpoint is introduced for this flow; this requirement exists only to document that the visible UI affordance has a defined backend route.

#### Scenario: Add-character link target
- **WHEN** the characters page renders the "+ add character" button
- **THEN** its `href` is `/auth/characters/add`
