<!-- All tasks shipped on `develop` in commit 1b95d69 prior to this change being
     written; checkboxes reflect the as-built state. -->

## 1. Member picker scope control

- [x] 1.1 Add a `scope` radio group to `MemberPicker.svelte` (`character` / `corporation` / `alliance` / `any`, default `any`) inside the existing `?/search` form, with paraglide labels and styling matching the design-token system.
- [x] 1.2 Co-located Vitest component tests: the four radios render, `any` is the default and submits as the `scope` field, and selecting a single category narrows the choice.

## 2. Search action mapping

- [x] 2.1 In `routes/acls/[id]/+page.server.ts`, read the `scope` form field in the `search` action and map it to the `categories` argument of `searchEntities(...)`: a single concrete category passes through; `any`/absent/unrecognized leaves `categories` undefined (backend all-categories default).
- [x] 2.2 Server-action tests: a single-category scope is forwarded as `categories`; `scope=any` and an absent scope both call `searchEntities` with `categories` undefined.

## 3. Internationalisation

- [x] 3.1 Add `picker_scope_legend`, `picker_scope_character`, `picker_scope_corporation`, `picker_scope_alliance`, `picker_scope_any` to `messages/en.json` and the SAME keys to `messages/de.json` and `messages/fr.json` (locale set kept in sync). Run paraglide compile from `frontend/`.

## 4. Verification

- [x] 4.1 `pnpm --filter frontend test` — Vitest unit/component tests (run as `pnpm test` from `frontend/`; this checkout has no workspace root, so the `--filter` form is unavailable). Result: 265 passing.
- [x] 4.2 `pnpm --filter frontend run check` — svelte-check type checking + paraglide compile (run as `pnpm run check` from `frontend/`). Result: 0 errors / 0 warnings.
- [x] 4.3 `pnpm --filter frontend run test:e2e` — Playwright e2e (run as `pnpm run test:e2e` from `frontend/`). Result: 20 passing.
