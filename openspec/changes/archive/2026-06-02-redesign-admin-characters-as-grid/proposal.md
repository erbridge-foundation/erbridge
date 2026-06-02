## Why

The admin Characters tab is search-first: nothing is shown until an admin types a name, and a character's account and token status are buried two interactions deep (search → "Inspect" modal). Yet `GET /api/v1/admin/accounts` already returns every account, every character, and each character's `token_status` up front — the search box and modal are client-side ceremony over data that is already fully loaded. Admins want to *see* the roster and triage token problems at a glance, not hunt for them one name at a time.

## What Changes

- **BREAKING (UI):** Replace the search box + results list + "Inspect" modal on `/admin/characters` with a client-side datagrid over the already-loaded `data.accounts`.
- One **row per account**, labelled by the account's main character's name (falling back to the first character by name if no character is flagged main).
- Each row rolls up the account's worst token state into an **Issues** column (counts of `token_expired` / `owner_mismatch`) so problems are visible without expanding.
- A row **expands** (`▸`/`▾`) to reveal the per-character token table — the same character/`token_status` table that lived in the modal, now inlined.
- **Hand-rolled, lightweight grid mechanics** (Svelte 5 `$derived`, no new dependency):
  - **Text filter** matching the main name *and* alt names (filtering by an alt surfaces its account row).
  - **Status chips** (All / Problems / Expired / Transferred) that filter at the account level — an account shows if any of its characters matches.
  - **Sortable column headers** (Account, Status, Admin, Issues severity, Created).
- **Remove** the ESI/character-search affordance from this page entirely: the `?/search` form action, the `searchCharacters` call, `CharacterSearchResultDto` usage, and the inspect modal/backdrop plumbing. Orphan/arbitrary-character ESI lookup remains owned by the block-character flow on `/admin/blocks`; this page is only ever about accounts that exist.
- **Remove** the now-dead i18n strings (`admin_characters_search_*`, `admin_characters_inspect`, `admin_characters_dialog_*`) from all four locale sources and add the new grid strings (column headers, filter chips, `+N alts`, issue counts) across `en`/`de`/`fr`, keeping the catalogue tight.
- **Rewrite** the page's unit/component and server tests against the grid (sort, text filter incl. alt-name match, status-chip filtering, expand/collapse, issues roll-up); drop the search-action server-test coverage.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `character-token-lifecycle`: the "Admin character search and token-state visibility" requirement changes — the admin Characters surface is no longer a search box + inspect dialog, but a datagrid (one row per account, expandable to per-character token state) with text filter, account-level status filtering, and sortable columns. The token-state visibility guarantee is preserved and strengthened (problems visible without interaction); the search/inspect-dialog mechanism is removed.

## Impact

- **Frontend only.** No backend change — `GET /api/v1/admin/accounts` and `AdminAccountDto` already supply everything the grid needs.
- `frontend/src/routes/admin/characters/+page.svelte` — full rewrite (grid replaces search + modal).
- `frontend/src/routes/admin/characters/+page.server.ts` — remove `actions.search`; keep the `listAdminAccounts` load.
- `frontend/src/routes/admin/characters/page.svelte.test.ts` and `page.server.test.ts` — rewritten.
- i18n message catalogue (`en`/`de`/`fr`) and the synced locale set — strings removed and added.
- No change to `frontend/src/lib/api.ts` types beyond no longer importing `CharacterSearchResultDto` on this page (`searchCharacters`/`CharacterSearchResultDto` remain for `/admin/blocks`).
