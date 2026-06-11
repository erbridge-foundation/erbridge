# Surface Add-Character Conflict

## Why

When a logged-in user completes the add-character SSO flow for a character that is already bound to a *different* account, the callback silently "succeeds": the character upsert deliberately keeps the existing binding, but the flow still redirects happily, refreshes the other account's stored tokens, and writes a `character_added` audit row attributing the character to the *session's* account — which is false. The user believes the character was added; it wasn't, and the audit trail says it was.

## What Changes

- The SSO callback's add-character path detects the already-bound-elsewhere case inside the completion transaction and treats it as a distinct outcome: no token overwrite on the other account's row, no `character_added` audit row, and a redirect to a conflict page/state the frontend renders (mirroring the existing `/blocked` redirect pattern).
- A truthful audit event (`character_add_rejected_bound_elsewhere`) records the rejected attempt, actor = the session account, target = the character.
- Frontend: the characters page (or a small interstitial route, matching how `/blocked` works) explains "this character is already linked to another account" with localisation in en/de/fr.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `eve-sso-auth`: the add-character callback contract gains the bound-elsewhere conflict outcome (currently the flow's behaviour in this case is unspecified and the implementation silently no-ops).
- `audit-log`: one new event type in the catalogue, `character_add_rejected_bound_elsewhere`.

## Impact

- Backend: `services/auth.rs` (`complete_sso_callback` add-character branch), `db/characters.rs` (`find_account_id_for_eve_character` already provides the needed lookup), `audit/mod.rs` (new event variant), `handlers/auth.rs` (conflict redirect).
- Frontend: conflict page/notice + i18n keys ×3 locales; e2e coverage in the mock-backend flow.
- No database schema changes (the audit catalogue is data, not schema).
