## Why

The EVE SSO access-token JWT carries an `owner` claim — a hash that CCP rotates whenever a character is transferred to a different EVE account (sold, traded, biomassed-and-restored). The callback already parses the JWT but discards this claim. Because we never compare it across logins, a character that has been transferred away keeps its tokens and its linkage to the previous owner's account: an account-theft / stale-access vector. We want to capture the owner hash and treat a change in it as a transfer event that severs the old owner's access.

## What Changes

- Persist the EVE SSO `owner` claim on each character as `eve_character.owner_hash`, set on every successful callback (first link, orphan-claim, and re-auth).
- On the OAuth2 callback, when an existing `eve_character` row is found whose stored `owner_hash` differs from the claim just presented, treat it as a **character transfer** and, in the same transaction, before the normal upsert:
  - Wipe the credential columns on **all** of the previous owner's characters (reusing the soft-delete token-wipe path), forcing that account to re-authorise every character.
  - Delete **all** of the previous owner's active session rows, so their cookie is dead and their next request forces a fresh login.
  - Detach the transferred character from the previous owner by setting `account_id = NULL` (the existing orphan state), clearing `is_main`, and storing the new `owner_hash`.
  - Emit an audit event recording the transfer (`eve_character_id`, old account, new account).
  - Fall through to the existing orphan-claim upsert path, which re-links the character to the authenticating account with fresh tokens.
- A first-seen character (no existing row) or a re-auth whose `owner_hash` is unchanged follows the existing callback behaviour unchanged, except that `owner_hash` is now recorded.

## Capabilities

### New Capabilities
- `character-ownership-tracking`: capturing the EVE SSO owner hash on every character, detecting a change as a transfer, and the enforcement that severs the previous owner's tokens and sessions and detaches the character.

### Modified Capabilities
- `eve-sso-auth`: the callback persistence rules gain the `owner_hash` field and the owner-hash-change branch in the "row exists with `account_id` set" case.

## Impact

- **Schema**: new nullable `owner_hash TEXT` column on `eve_character` (new migration). No change to `account`.
- **Backend (Rust)**:
  - `handlers/auth.rs` — parse the `owner` claim into `EsiJwtClaims`; thread it into the service input.
  - `services/auth.rs` — owner-hash comparison and transfer-enforcement orchestration in the existing callback transaction.
  - `db/characters.rs` — extend the existing eve-character lookup to return `owner_hash`; persist `owner_hash` on insert/upsert/orphan paths. Reuses existing `clear_tokens_for_account`.
  - `db/sessions.rs` — reuses existing `delete_for_account`.
  - `audit/mod.rs` — new `CharacterTransferDetected` (owner-hash-change) event variant, in the established house style.
- **No frontend changes.** The previous owner is simply signed out and re-authenticates through the normal flow; they performed the transfer deliberately, so no in-app notice is needed.
- **Out of scope (follow-up)**: cleanup of the previous owner's map/ACL entries that referenced the transferred character. The audit event makes the transfer visible in the meantime.
- **Tooling**: schema/query changes require regenerating the sqlx offline cache (`cargo sqlx prepare -- --all-targets`).
