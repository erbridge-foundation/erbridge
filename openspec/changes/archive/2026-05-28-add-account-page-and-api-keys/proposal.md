## Why

The backend ships `POST/GET/DELETE /api/v1/keys` per the `api-authentication` capability, with DTOs already declared in `frontend/src/lib/api.ts` — but the frontend has no UI to call them. Users who want a programmatic credential for E-R Bridge today cannot create one without hitting the API directly. This change ships the missing frontend surface.

The user-chip menu has carried a disabled "Settings" item since the menu was first added — a deliberate stake for the future account surface. This change activates that affordance, names it correctly ("Account"), and points it at a new `/account` page that hosts API key management plus the account-lifecycle controls that currently live awkwardly at the bottom of `/characters`. The placement of "Delete account" on `/characters` is a category mismatch (account deletion is not character management); moving it as part of this change reduces the surface left to clean up later.

## What Changes

- **New `/account` page** (SvelteKit SSR loader + form actions, mirroring `/characters`):
  - Tabs (consistent with `/preferences`): `[ API keys ] [ Danger zone ]`.
  - **API keys tab**: list (name / created / expires / [Revoke]) with expired keys shown de-emphasised; empty state with explanatory copy and a "Create your first" CTA; inline create form with name input + expiry presets (Never / 30 days / 90 days / 1 year / Custom date); **one-shot inline reveal panel** for the freshly-created plaintext key, gated by a "I've saved this key somewhere safe" checkbox that enables the [Done] button (the only time the plaintext is ever surfaced, per `api-authentication`); revoke uses `ConfirmDialog` per `frontend-patterns`.
  - **Danger zone tab**: hosts the moved "Delete account" action.
- **Move "Delete account" from `/characters` to `/account`**: the `?/deleteAccount` form action, its `ConfirmDialog`, and the trigger button. **BREAKING** for any external link or bookmark pointing into the `/characters` danger-zone section (internal app; documented as acceptable).
- **Enable the user-chip menu item**: `UserMenu.svelte`'s currently-disabled span becomes `<a href="/account">`. i18n key renamed `user_menu_settings` → `user_menu_account`; values changed (en: "settings" → "account"; de: "Konfiguration" → "Konto"). **BREAKING** for the message key; no external consumer of paraglide keys exists outside the frontend bundle.
- **API client functions**: add `listKeys`, `createKey`, `deleteKey` to `frontend/src/lib/api.ts`, following the existing `request<T>()` cookie-forwarding pattern. DTOs are already declared — no shape changes.
- **i18n**: new strings for the `/account` page (tabs, list headers, empty state, create form, reveal-panel copy, revoke confirmation). Move `characters_delete_account*` strings to an `account_delete_account*` namespace to reflect their new home.
- **Tests**: new `frontend/src/routes/account/page.server.test.ts` covering create / list / revoke / delete-account form actions in isolation; component-level coverage of the inline reveal gate (Done disabled until checkbox); regression on `/characters` that the delete-account UI is gone.

Out of scope: a "Sessions" tab (defer until SSE map-events work introduces session-as-connection semantics); server-scoped API key management (out-of-band per `api-authentication`); "last used at" telemetry on keys (would require a backend change); renaming `/preferences` (preferences ≠ account; preferences stays as a sibling).

## Capabilities

### New Capabilities

- `account-page`: frontend requirements for `/account` — the tabbed page structure, API-key list/create/revoke flows including the one-shot plaintext reveal gate, and the danger-zone tab that hosts the delete-account action. Precedent for one-frontend-page-per-capability is set by `about-page`.

### Modified Capabilities

- `account-management`: minor delta only if the existing spec references `/characters` as the host for the delete-account UI affordance. The HTTP contract for `DELETE /api/v1/account` is unchanged; only the documented UI host moves.
- `internationalisation`: deltas for the renamed `user_menu_settings` key and the moved `characters_delete_account*` keys, plus the new `account_*` strings.

## Impact

- **Frontend**: new route `frontend/src/routes/account/+page.{server.ts,svelte}`, new component(s) under `frontend/src/lib/components/` for the API-keys section (a reveal panel that owns the plaintext-survival state), additions to `frontend/src/lib/api.ts`, edits to `frontend/src/lib/components/UserMenu.svelte`, removals from `frontend/src/routes/characters/+page.{server.ts,svelte}`, message updates in `frontend/messages/en.json` and `frontend/messages/de.json`.
- **Backend**: small bundled fix — the `api-authentication` spec mandates HTTP 409 on duplicate API-key name per account, but the `api_key` schema was missing the unique constraint that backs that rule (the service layer already maps `DbError::UniqueViolation` → `ConflictKind::ApiKeyNameAlreadyExists` → 409, but no constraint was firing). Adds migration `00000000000004_api_key_unique_name_per_scope.sql` (partial unique indexes per scope: `(account_id, name) WHERE scope='account'` and `(name) WHERE scope='server'`), DB unit tests, integration tests, and a HURL step. No service/handler/DTO changes — only the schema gap is closed. Surfaced during manual verification of the new UI; fixed inline because the UI's inline-error path explicitly depends on the 409 firing.
- **Tests**: new route-level tests for `/account`; new component tests for the reveal gate; existing `/characters` tests updated to remove the moved cases.
- **Patterns consumed (not modified)**: `frontend-patterns` (`ConfirmDialog` for revoke and delete-account), `api-authentication` (the plaintext-key one-shot invariant), `sveltekit-node` skill (authoritative frontend layout).
