## 1. i18n

- [x] 1.1 Add `admin_blocks_self_badge` (e.g. "You") and `admin_blocks_self_title` (e.g. "One of your own characters — you can't block yourself") to `frontend/messages/en.json`.
- [x] 1.2 Add the same two keys with translations to `frontend/messages/de.json` and `frontend/messages/fr.json` (all three locales must stay in sync).
- [x] 1.3 Add `admin_admins_revoke_self_warning` (e.g. "This is your own account — you will lose your admin access immediately.") to `frontend/messages/{en,de,fr}.json` (all three locales).

## 2. Picker affordance

- [x] 2.1 In `frontend/src/routes/admin/blocks/+page.svelte`, derive the current account from layout data (`let me = $derived(data.me)`) and add an `isSelf(result)` helper: local results (`'account_id' in result`) match on `result.account_id === me.account.id`; ESI results match on `me.characters.some(c => c.eve_character_id === result.eve_character_id)`. Guard for `me` being null (fall back to not-self).
- [x] 2.2 In the `resultList` snippet, add a third branch between `already_blocked` and the Select form: `{:else if isSelf(result)}` renders `<span class="self-badge" title={m.admin_blocks_self_title()}>{m.admin_blocks_self_badge()}</span>` and no Select form.
- [x] 2.3 Add a `.self-badge` style cloned from `.blocked-badge` but using a neutral slate tone (border + text), not red.

## 3. Revoke self-warning

The backend deliberately *permits* self-revoke (guarded only by the last-admin rule), so this is a warning, **not** a block: the admin can still revoke their own rights.

- [x] 3.1 In `frontend/src/routes/admin/admins/+page.svelte`, derive `let me = $derived(data.me)` and add `isSelf(account)` (`account.id === me.account.id`, null-guarded).
- [x] 3.2 In the revoke `ConfirmDialog` `body` snippet, when `isSelf(revokeState.account)` also render `<span class="self-revoke-warning" role="alert">{m.admin_admins_revoke_self_warning()}</span>` after the existing body text. The Revoke button is unchanged (self-revoke stays possible).
- [x] 3.3 Add a `.self-revoke-warning` style — amber/yellow notice (`display:block` so it is a valid inline child of the body `<p>`), modelled on the blocks page `.notice`.

## 4. Tests

- [x] 4.1 In `frontend/src/routes/admin/blocks/page.svelte.test.ts`, add component coverage: a local result whose `account_id` matches `data.me.account.id` renders the self badge and no Select control; an ESI result whose `eve_character_id` matches one of `data.me.characters` renders the self badge; a result on another account renders the normal Select control. Ensure the test setup provides `data.me`.
- [x] 4.2 In `frontend/src/routes/admin/admins/page.svelte.test.ts`, add component coverage: opening the revoke dialog for the admin's own account (`data.me.account.id`) shows the self-revoke warning; opening it for another account does not. Ensure the test setup provides `data.me`.

## 5. Verification

- [x] 5.1 From `frontend/`, run `pnpm test` (Vitest) — all pass.
- [x] 5.2 From `frontend/`, run `pnpm run check` (svelte-check + paraglide compile) — 0 errors, 0 warnings.
- [x] 5.3 From `frontend/`, run `pnpm run test:e2e` (Playwright) — all pass.
