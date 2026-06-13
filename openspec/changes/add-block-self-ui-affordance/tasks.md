## 1. i18n

- [ ] 1.1 Add `admin_blocks_self_badge` (e.g. "You") and `admin_blocks_self_title` (e.g. "One of your own characters — you can't block yourself") to `frontend/messages/en.json`.
- [ ] 1.2 Add the same two keys with translations to `frontend/messages/de.json` and `frontend/messages/fr.json` (all three locales must stay in sync).

## 2. Picker affordance

- [ ] 2.1 In `frontend/src/routes/admin/blocks/+page.svelte`, derive the current account from layout data (`let me = $derived(data.me)`) and add an `isSelf(result)` helper: local results (`'account_id' in result`) match on `result.account_id === me.account.id`; ESI results match on `me.characters.some(c => c.eve_character_id === result.eve_character_id)`. Guard for `me` being null (fall back to not-self).
- [ ] 2.2 In the `resultList` snippet, add a third branch between `already_blocked` and the Select form: `{:else if isSelf(result)}` renders `<span class="self-badge" title={m.admin_blocks_self_title()}>{m.admin_blocks_self_badge()}</span>` and no Select form.
- [ ] 2.3 Add a `.self-badge` style cloned from `.blocked-badge` but using a neutral slate tone (border + text), not red.

## 3. Tests

- [ ] 3.1 In `frontend/src/routes/admin/blocks/page.svelte.test.ts`, add component coverage: a local result whose `account_id` matches `data.me.account.id` renders the self badge and no Select control; an ESI result whose `eve_character_id` matches one of `data.me.characters` renders the self badge; a result on another account renders the normal Select control. Ensure the test setup provides `data.me`.

## 4. Verification

- [ ] 4.1 From `frontend/`, run `pnpm test` (Vitest) — all pass.
- [ ] 4.2 From `frontend/`, run `pnpm run check` (svelte-check + paraglide compile) — 0 errors, 0 warnings.
- [ ] 4.3 From `frontend/`, run `pnpm run test:e2e` (Playwright) — all pass.
