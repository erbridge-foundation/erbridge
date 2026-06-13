## Why

The backend already rejects an admin's attempt to block a character on their own account (`409 cannot_block_self`), but the block picker UI gives no hint: the admin's own characters render as fully selectable "Select" buttons, so the only feedback is a failed action *after* selecting, looking up the corp, and confirming. The picker should mark those characters as non-selectable up front, the same way it already marks already-blocked pilots.

## What Changes

- In the admin block picker (`/admin/blocks`), search results that belong to the logged-in admin render a non-actionable **"You"** badge instead of a Select button — mirroring the existing `already_blocked` badge pattern.
- A result counts as "self" using the same account-level semantics the backend enforces:
  - **Local** results: `account_id` matches the current account's id (catches every alt on the admin's account).
  - **ESI** results (no `account_id`): the result's `eve_character_id` matches one of the current account's characters.
- The badge carries a short explanatory `title` ("one of your own characters — you can't block yourself") so the reason is discoverable on hover.
- In the admin **revoke** picker (`/admin/admins`), revoking your *own* account stays possible (the backend deliberately permits self-revoke, guarded only by the last-admin rule) but the revoke confirmation dialog gains a yellow warning banner highlighting that you are about to revoke your own admin rights. This is a footgun guard, **not** a block.
- No backend change. The backend guard remains the enforcement boundary; this is presentation-layer defence-in-depth (block) / a confirmation warning (revoke).

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `server-administration`: the self-block rule gains a UI-affordance requirement — the block picker SHALL visibly mark the admin's own characters as non-selectable (no new behaviour on the request path; the existing `409 cannot_block_self` guard is unchanged). Additionally, the admin revoke confirmation SHALL warn when an admin is about to revoke their own admin rights — self-revoke remains permitted (the backend's deliberate behaviour), so this is a warning, not a block.

## Impact

- **Frontend only.**
  - `frontend/src/routes/admin/blocks/+page.svelte` — `isSelf` check against `data.me`, a third render branch in the `resultList` snippet, and a `.self-badge` style.
  - `frontend/src/routes/admin/admins/+page.svelte` — `isSelf` check against `data.me`, a conditional warning banner in the revoke `ConfirmDialog` body, and a `.self-revoke-warning` style.
  - `frontend/messages/{en,de,fr}.json` — new i18n keys for the badge label/title and the revoke self-warning (all three locales, per repo rule).
  - `frontend/src/routes/admin/blocks/page.svelte.test.ts` and `frontend/src/routes/admin/admins/page.svelte.test.ts` — component coverage for the self badge and the self-revoke warning.
- No API, DB, or backend service changes. No new dependencies.
