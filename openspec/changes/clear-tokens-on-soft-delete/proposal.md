## Why

When an account is soft-deleted (`DELETE /api/v1/account`), the backend sets `account.status = 'soft_deleted'` and `delete_requested_at = now()` but **leaves `eve_character.encrypted_access_token` and `eve_character.encrypted_refresh_token` intact**. The encrypted token material remains on disk for the entire soft-delete window (currently undefined-but-discussed-as-30-days), protected only by `encryption_secret`.

This is a spec gap, not a code bug: the `account-management` capability says "character rows SHALL NOT be modified" on soft-delete, which a literal read interprets as "don't touch their columns either." Whether that interpretation is the right one is the open question.

There are competing concerns:

- **Argument for clearing tokens on soft-delete.** A user who soft-deletes their account because of a security incident (compromised laptop, leaked DB snapshot, etc.) reasonably expects their EVE credentials to be revoked from this service. Keeping encrypted-but-functional tokens on disk for ~30 days weakens that.
- **Argument against (the current behaviour).** Soft-delete is explicitly reversible per spec: a subsequent SSO login reactivates the account. If tokens are cleared on soft-delete, every linked character's `token_status` flips to `"expired"` on reactivation, forcing the user to re-do SSO for every character. That's a noisy UX for the "I changed my mind" happy path that soft-delete is *meant* to support.
- **Possible middle ground.** Clear only `encrypted_access_token` (short-lived, ~20 min) on soft-delete and keep `encrypted_refresh_token` (the long-lived credential the spec already treats as the "is this character usable" signal). This matches the spec's existing rule that `token_status` is derived from the *refresh* token's presence, not the access token's. It also matches what an attacker would care about: an access token is useless after 20 minutes; a refresh token is the durable credential.

This change is a **stub** — it captures the question so it isn't lost. The actual decision (clear both / clear neither / clear access-only / distinguish user-initiated vs admin-initiated) belongs in design.md after an `/opsx:explore` session.

## What Changes

To be decided in design.md. Candidate scopes:

- **MODIFIED** `account-management`: change the "Character rows SHALL NOT be modified" rule on `DELETE /api/v1/account` to explicitly state which columns are zeroed and which are kept. Add scenarios covering the chosen behaviour on soft-delete and on reactivation via re-login.

## Capabilities

### Modified Capabilities

- `account-management`: the `DELETE /api/v1/account` requirement needs an explicit position on encrypted token columns. The current "SHALL NOT be modified" is interpreted as "leave tokens intact" today; this change either reaffirms that with explicit reasoning, or narrows it to specific columns.

## Impact

- **Backend**: `services/account.rs::delete_account` — likely a column-clearing statement on `eve_character` for the soft-deleted account.
- **Spec**: `account-management` capability text.
- **Frontend**: probably no change (the `/me` response's `token_status` will reflect cleared tokens automatically per the existing derivation rule).
- **Reactivation UX**: depending on the decision, users who soft-delete and re-login may need to walk through SSO again for some/all characters.

## Status

**Stub.** This proposal exists to keep the question discoverable. Surfaced during the `add-account-page-and-api-keys` change while moving the delete-account UI to `/account`. Do not implement without an explore session and a position recorded in `design.md`.
