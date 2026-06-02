## 1. i18n strings

- [x] 1.1 Remove the now-dead `admin_characters_*` keys exclusive to search/modal (`admin_characters_search_*`, `admin_characters_inspect`, `admin_characters_search_empty`, `admin_characters_search_orphan`, `admin_characters_dialog_*`, and `admin_characters_filter_*` if unused) from the `en`/`de`/`fr` message sources — grep usages first to confirm none are referenced elsewhere
- [x] 1.2 Add the new grid keys (page heading/intro if changed, column headers: account/status/admin/issues/created; filter chips: all/problems/expired/transferred; text-filter placeholder + aria; `+N alts`; issue counts for expired/transferred; expand/collapse aria) across `en`/`de`/`fr`, keeping the catalogue tight
- [x] 1.3 Keep the `admin_characters_token_*` labels (still used by the per-character table)
- [x] 1.4 Run Paraglide compile from `frontend/` (not via `--filter`) and confirm the locale set stays synced

## 2. Server load

- [x] 2.1 In `frontend/src/routes/admin/characters/+page.server.ts`, remove `actions.search`, the `searchCharacters` import, and the `ApiError`/`fail` imports if no longer used; keep the `listAdminAccounts` load that forwards the cookie and returns `{ accounts }`

## 3. Grid page

- [x] 3.1 Rewrite `frontend/src/routes/admin/characters/+page.svelte`: remove the search form, results list, and inspect modal/backdrop plumbing (`inspect` state, `openInspect`/`closeInspect`, `accountsById`, `CharacterSearchResultDto` import)
- [x] 3.2 Add account-label derivation: main character (`is_main`) else first character by name; expose alt count (`+N`)
- [x] 3.3 Add the Issues roll-up per account (counts of `token_expired` and `owner_mismatch`; `—` when all active) reusing existing token-state colour tokens
- [x] 3.4 Render the datagrid: one row per account with columns Account / Status / Admin / Alts(`+N`) / Issues / Created, plus a `▸`/`▾` expand control
- [x] 3.5 Inline the per-character token table (reused from the old modal: token-status dots/colours, `tokenLabel()`, main badge) into the expanded row, driven by a `$state` expanded-id (or `Set`)
- [x] 3.6 Add the free-text filter (`$state` + `$derived` predicate matching main name OR any alt name, case-insensitive)
- [x] 3.7 Add account-level status chips (`all` / `problems` / `expired` / `transferred`) as a `$derived` filter
- [x] 3.8 Add sortable column headers (`$state` `{ column, dir }` + `$derived` sorted list) for Account, Status, Admin, Issues severity, Created; click toggles asc/desc

## 4. Tests (first-class)

- [x] 4.1 Rewrite `frontend/src/routes/admin/characters/page.server.test.ts`: load returns `accounts` from `listAdminAccounts` with the cookie forwarded; delete all `?/search` action coverage
- [x] 4.2 Rewrite `frontend/src/routes/admin/characters/page.svelte.test.ts`: a row per account labelled by main; first-character fallback when no `is_main`; Issues roll-up visible while collapsed
- [x] 4.3 Component tests: expand/collapse reveals/hides the per-character token table
- [x] 4.4 Component tests: text filter matches both main and alt names (alt match surfaces the account row)
- [x] 4.5 Component tests: status chips filter at account level (`problems`/`expired`/`transferred`)
- [x] 4.6 Component tests: column sort toggles order for at least Account and Issues
- [x] 4.7 Update any Playwright e2e spec that drove the old search/modal flow on `/admin/characters` to the grid (filter/expand selectors)

## 5. Verification

<!-- Note: this repo has no root pnpm workspace manifest, so the three commands are
     run from `frontend/` (e.g. `pnpm run test`), NOT via `--filter frontend`. -->
- [x] 5.1 `pnpm run test` (from `frontend/`) — Vitest unit/component tests pass (182 passed)
- [x] 5.2 `pnpm run check` (from `frontend/`) — svelte-check (type checking + Paraglide compile) passes with no dangling message references (0 errors, 0 warnings)
- [x] 5.3 `pnpm run test:e2e` (from `frontend/`) — Playwright e2e tests pass (17 passed)
