## Why

When an account is soft-deleted (`DELETE /api/v1/account`), the backend sets `account.status = 'soft_deleted'` and `delete_requested_at = now()` but leaves `eve_character.encrypted_access_token` and `eve_character.encrypted_refresh_token` intact. The encrypted token material remains on disk for the entire soft-delete window — protected only by `encryption_secret` — until hard-purge runs.

The user's mental model when they click "delete account" is "my credentials are gone from this service." The current behaviour silently violates that: an attacker with the database and `encryption_secret` retains usable refresh tokens for every linked character throughout the window. The reactivation-friction argument for keeping tokens (one click of SSO restores everything seamlessly) is also weaker than it looks — re-login only refreshes tokens for the *one* character that logged in; other alts already may or may not work depending on EVE-side state.

This change makes the spec say what users intuit: soft-delete clears our copy of the EVE credentials, framed in terms of the soft-delete *event* rather than the actor that triggered it, so the rule survives any future admin-initiated or system-initiated soft-delete path.

## What Changes

- **MODIFIED** `account-management`: the `DELETE /api/v1/account` requirement is rewritten to specify two things in one pass, both motivated by the same "delete = gone" mental model:
  1. Whenever an `account` row transitions to `status = 'soft_deleted'`, every linked `eve_character` row has `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, and `scopes` set to NULL (or empty array for `scopes`) in the same transaction. The "Character rows SHALL NOT be modified" wording is removed.
  2. The response carries EXACTLY ONE `Set-Cookie` header — the cookie-clearing one. The session-refresh middleware MUST NOT also emit a refreshed session cookie on this response (the pre-existing implementation did, leaving the browser logged in until the next request failed).

  Scenarios cover the soft-delete write, the post-reactivation read (other alts come back as `token_status = "expired"`), the explicit non-promise that EVE-side app authorisation is revoked, and the single-Set-Cookie response shape.

## Capabilities

### Modified Capabilities

- `account-management`: `DELETE /api/v1/account` requirement and its scenarios.

## Impact

- **Backend**:
  - `backend/src/db/accounts.rs` — `soft_delete` signature changes from `&PgPool` to `&mut Transaction<'_, Postgres>` so the account-status write and the character-token clear are atomic.
  - `backend/src/db/characters.rs` — new `clear_tokens_for_account(tx, account_id)` that nulls the four columns for every `eve_character` row owned by the account.
  - `backend/src/services/account.rs::delete_account` — opens a transaction, calls `soft_delete` then `clear_tokens_for_account`, commits.
  - `backend/src/handlers/middleware.rs` — `RefreshedJwtSlot` becomes `pub` and gains a `pub fn suppress(&self)` that empties the slot so the wrapping `refresh_session_cookie` middleware writes no `Set-Cookie`. Tiny API surface increase; type was already in `request.extensions`.
  - `backend/src/handlers/api/v1/account.rs::delete_account` — extracts `Extension<RefreshedJwtSlot>` and calls `.suppress()` after the service call succeeds so the browser cookie clear is not overwritten by a refreshed session cookie.
  - Tests: existing `delete_account_*` sqlx tests gain assertions on the character-token columns; new test that an account with characters has all four columns nulled after `delete_account`; new integration test on `DELETE /api/v1/account` asserts exactly one `Set-Cookie` header on the response and that it clears the session (`Max-Age=0`); unit tests on `RefreshedJwtSlot::suppress`.
- **Frontend**: no code changes. The `/me` response's `token_status` derives from refresh-token presence and will correctly report `"expired"` for every character of a reactivated account until each is re-SSO'd. The account page already surfaces per-character re-auth affordances.
- **Reactivation UX**: a user who soft-deletes and re-logs in must walk through SSO once per character (other than the one that logged in to reactivate) to restore full functionality. This is an intentional trade for the cleaner security story.
- **Out of scope**: calling EVE's SSO token-revocation endpoint. The spec explicitly notes that soft-delete clears *our* copy only and does not revoke the app's EVE-side authorisation; users concerned about credential compromise are directed to revoke authorisation at EVE SSO themselves. A future change may add server-side revocation as a follow-up.
