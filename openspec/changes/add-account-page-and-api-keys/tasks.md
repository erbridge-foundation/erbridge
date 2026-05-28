## 1. API client functions

- [x] 1.1 In `frontend/src/lib/api.ts`, add `listKeys(fetch, backendUrl, cookie): Promise<KeyMetadataDto[]>` calling `GET /api/v1/keys` via the existing `request<T>()` helper
- [x] 1.2 Add `createKey(fetch, backendUrl, body: CreateKeyRequest, cookie): Promise<CreatedKeyDto>` calling `POST /api/v1/keys` with `content-type: application/json` and the JSON-stringified body
- [x] 1.3 Add `deleteKey(fetch, backendUrl, keyId: string, cookie): Promise<void>` calling `DELETE /api/v1/keys/:id`
- [x] 1.4 Extend `frontend/src/lib/api.test.ts` to cover the new functions (success envelope unwrap, 204 handling for delete, `ApiError` mapping)

## 2. i18n catalogue updates

- [x] 2.1 In `frontend/messages/en.json` and `de.json`, rename `user_menu_settings` → `user_menu_account`; change values en `"settings"` → `"account"`, de `"Konfiguration"` → `"Konto"`
- [x] 2.2 In both catalogues, rename `characters_delete_account` → `account_delete_account`, `characters_delete_account_title` → `account_delete_account_title`, `characters_delete_account_body` → `account_delete_account_body`, `characters_delete_account_confirm` → `account_delete_account_confirm`, `characters_danger_zone` → `account_danger_zone`. Values unchanged.
- [x] 2.3 Add new `account_*` strings for the API keys section (tab label, list headers, empty-state copy + CTA, `[+ New key]`, name input label/placeholder, expiry preset labels (`Never` / `In 30 days` / `In 90 days` / `In 1 year` / `Custom...`), reveal panel warning + checkbox label + `[Copy]` + `[Done]`, revoke button + confirmation dialog title/body/confirm). Mirror keys across `en.json` and `de.json`.
- [x] 2.4 Run the paraglide build step (`pnpm --filter frontend run …` as configured) and confirm no orphan-key or missing-key warnings; `svelte-check` is the authoritative pass

## 3. User menu

- [x] 3.1 In `frontend/src/lib/components/UserMenu.svelte`, replace the disabled `<span class="item disabled" …>` for the settings placeholder with `<a class="item" href="/account" role="menuitem" onclick={onclose}>{m.user_menu_account()}</a>`
- [x] 3.2 Remove the `.item.disabled` CSS rules iff no other disabled items remain (visual sweep); otherwise leave them

## 4. /account route — server side

- [x] 4.1 Create `frontend/src/routes/account/+page.server.ts`. Implement `load` that fetches `listKeys(...)` (cookie forwarded via `backend_internal_url()` pattern, matching `/characters` and `/preferences`) and returns `{ keys: KeyMetadataDto[] }`
- [x] 4.2 Define `actions` with three entries:
  - `createKey`: read `name` and `expires_at` from `formData`; validate `name` non-empty; call `createKey(...)`; on success return `{ createdKey: CreatedKeyDto }`; on `ApiError` return `fail(status, { code, message })`
  - `revokeKey`: read `key_id` from `formData`; call `deleteKey(...)`; on `ApiError` return `fail(status, { keyId, code, message })` so the page can render an inline error keyed to the row
  - `deleteAccount`: lift verbatim from `frontend/src/routes/characters/+page.server.ts` — same `deleteAccount(...)` call, same clear-session-cookie behaviour, same redirect to `/login`
- [x] 4.3 Add `frontend/src/routes/account/page.server.test.ts` covering each action (happy path + error paths), mirroring the shape of `frontend/src/routes/characters/page.server.test.ts`

## 5. /account route — client side

- [x] 5.1 Create `frontend/src/routes/account/+page.svelte` with a `<Tabs>`-style local state (`let activeTab = $state<'api-keys' | 'danger-zone'>('api-keys')`), reusing the visual pattern from `/preferences` (tab strip → conditional content)
- [x] 5.2 API keys tab — list section: render rows for `data.keys` showing name / created (formatted) / expires (formatted date or "Never" or "Expired" badge) / `[Revoke]` button. De-emphasise (token-driven foreground colour) rows whose `expires_at` is in the past
- [x] 5.3 API keys tab — empty state: when `data.keys.length === 0`, render explanatory copy + a `[Create your first]` button that toggles the create form open. Use the same form component as the populated view, just relabelled
- [x] 5.4 API keys tab — create form: name input + expiry preset selector (`Never`/`30d`/`90d`/`1y`/`Custom...`). On Custom, reveal `<input type="date">`. On submit, compute `expires_at` (RFC3339; end-of-day UTC for custom; `null` for Never). Use `<form method="POST" action="?/createKey" use:enhance>`. Client-side validate non-empty name; on success the page re-loads via SvelteKit and the new row appears
- [x] 5.5 API keys tab — reveal panel: introduce `let revealedKey = $state<CreatedKeyDto | null>(null)`. Use an `$effect` to copy `form?.createdKey` into `revealedKey` (then trigger `invalidateAll()`). Render the panel from `revealedKey`, NOT from `form`. Panel contains: warning copy, plaintext `<code>` (selectable), `[Copy]` button (`navigator.clipboard.writeText` with try/catch + inline "select manually" fallback), `[ ] I've saved this key somewhere safe` checkbox bound to `let acknowledged = $state(false)`, `[Done]` button with `disabled={!acknowledged}` that sets `revealedKey = null` and `acknowledged = false`
- [x] 5.6 API keys tab — revoke flow: per-row `<form bind:this={revokeForms[key.id]} method="POST" action="?/revokeKey" use:enhance>` with hidden `key_id`; `[Revoke]` button is `type="button"` and opens `ConfirmDialog` (state: `let revokeState = $state<{ open, key } | null>(null)`); dialog `onConfirm` calls `revokeForms[key.id]?.requestSubmit()`. Render inline error row when `form?.keyId === key.id && form?.code`, matching the `/characters` `formError?.characterId` pattern
- [x] 5.7 Danger zone tab: lift the delete-account button + `ConfirmDialog` + `<form method="POST" action="?/deleteAccount">` from `/characters` verbatim, swap message keys to `m.account_delete_account*()`, and keep the same `bind:this` + `requestSubmit()` shape
- [x] 5.8 Component test (svelte testing): `+page.svelte` reveal-panel coverage — Done disabled until checkbox ticked; activating Done unmounts the panel and clears the plaintext from the DOM; clipboard-write rejection renders the manual-select hint without breaking the gate. Per the `frontend-patterns` precedent, do NOT re-test `ConfirmDialog` here (it has its own tests)

## 6. /characters cleanup

- [x] 6.1 In `frontend/src/routes/characters/+page.server.ts`, remove the `deleteAccount` action and its import of `deleteAccount` from `$lib/api` (the function in `$lib/api.ts` stays — it's now called from `/account`)
- [x] 6.2 In `frontend/src/routes/characters/+page.svelte`, remove the danger-zone heading, divider, delete-account button, `ConfirmDialog` for delete-account, the `deleteAccountOpen` / `deleteAccountForm` state, and the `m.characters_delete_account*()` / `m.characters_danger_zone()` calls
- [x] 6.3 Update `frontend/src/routes/characters/page.server.test.ts` to remove the `deleteAccount` action cases (they move to `/account`'s test file)
- [x] 6.4 Regression check in `/characters` tests (or a new assertion): grep-style assertion that `?/deleteAccount` and `characters_delete_account` no longer appear in the route's source

## 8. Backend inline fix — enforce unique key name per account/server (added during verification)

The `api-authentication` spec requires HTTP 409 on duplicate name per account; the service layer already returns 409 on `DbError::UniqueViolation`, but the constraint was missing from the schema. Surfaced during manual `/account` verification — the UI's inline error path depends on this 409 firing.

- [x] 8.1 Add migration `backend/migrations/00000000000004_api_key_unique_name_per_scope.sql` with two partial unique indexes: `(account_id, name) WHERE scope='account'` and `(name) WHERE scope='server'`
- [x] 8.2 DB unit tests in `backend/src/db/api_keys.rs`: `insert_duplicate_name_same_account_returns_unique_violation`, `insert_same_name_different_accounts_succeeds`
- [x] 8.3 Integration tests in `backend/tests/api_keys.rs`: `create_key_duplicate_name_returns_409`, `create_key_same_name_different_accounts_returns_201`
- [x] 8.4 HURL test step in `backend/tests/hurl/keys.hurl` asserting 409 + `api_key_name_already_exists` error code on duplicate name
- [x] 8.5 Regenerate sqlx offline cache (`cargo sqlx prepare -- --all-targets`) and commit `.sqlx/` diff
- [x] 8.6 Verify in dev browser: duplicate name surfaces as an inline error in the create form (verified — screenshot evidence in conversation)

## 7. Verification

- [x] 7.1 Run `pnpm --filter frontend test` and `pnpm --filter frontend run check` (svelte-check) — both pass
- [x] 7.2 Manual browser walk-through against the dev stack (`docker compose -f docker-compose.dev.yml`):
  - [x] Open `/account` from the user-chip menu — menu item reads "account", lands on `/account` with API keys tab active
  - [x] Create a key with name "test", Never expiry — list refreshes, reveal panel appears with plaintext (basic create + reveal path)
  - [x] Duplicate-name create surfaces inline 409 with "A key with this name already exists" (verifies the inline-backend-fix bundled into this change)
  - [x] Danger zone → Delete account opens the confirmation dialog with the moved copy (UI half — destructive confirm not exercised)
  - [x] Empty state renders the explanatory copy + `[Create your first]` CTA on a fresh account
  - [x] Create a key with each remaining expiry preset (30d / 90d / 1y / Custom date) — reveal panel appears for each
  - [x] `[Copy]` writes to clipboard; `[Done]` is disabled until the checkbox is ticked; activating Done unmounts the panel and the plaintext is no longer in the DOM
  - [x] Reloading the page mid-reveal does NOT bring the panel back (no sessionStorage persistence)
  - [x] Revoke a key — `ConfirmDialog` opens, Cancel leaves it intact, Confirm removes the row
  - [x] Force an expired key (via `UPDATE api_key SET expires_at = now() - interval '1 day' WHERE name = 'test'` against the dev DB) — row renders de-emphasised with the Expired badge; revoke still works
  - [x] Confirm Delete account end-to-end (destructive — clears session, lands on `/login`; reactivate via re-login)
- [x] 7.3 Confirm `/characters` no longer shows the danger zone heading, divider, or delete-account button; the page still functions for character management
- [x] 7.4 Confirm the German locale renders all new and renamed strings (toggle locale via `/preferences`)
- [x] 7.5 Run `openspec validate add-account-page-and-api-keys --strict` and ensure it passes
