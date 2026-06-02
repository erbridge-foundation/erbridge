## Context

`/admin/characters` today is a search-first page. `+page.server.ts` loads the full admin accounts list (`listAdminAccounts` → `GET /api/v1/admin/accounts`, returning `AdminAccountDto[]` with each account's `id`, `status`, `is_server_admin`, `created_at`, and `characters[]` carrying `is_main` + `token_status`). Despite that, the page shows nothing until the admin submits the `?/search` form action, which calls `searchCharacters`; results render as a flat list, and an "Inspect" button opens a modal that — using the *already-loaded* accounts indexed by id — shows the selected character's account and per-character token state.

So the data the admin wants is fully present on load; the search box, the `?/search` action, and the modal are client/server ceremony layered over it. The block-character flow on `/admin/blocks` already owns ESI/orphan lookup (local→ESI fallback), so this page never needs to find a character outside the known roster.

Constraints: `sveltekit-node` skill governs frontend structure and bans heavy UI dependencies (native CSS, Svelte 5 runes, no Tailwind/CSS-in-JS). CLAUDE.md requires the full three-command frontend gate (vitest + `check` + e2e) before completion, and treats tests as first-class. The i18n catalogue (Paraglide) must stay tight — dead strings removed, new strings added across the synced `en`/`de`/`fr` set.

## Goals / Non-Goals

**Goals:**

- Show the full account roster on load as a datagrid, one row per account, with token problems visible without interaction.
- Preserve and strengthen the token-state visibility guarantee from the existing `character-token-lifecycle` requirement.
- Keep mechanics lightweight and dependency-free (Svelte 5 `$derived` sort/filter/expand).
- Remove the search/inspect surface and its now-dead i18n strings cleanly.
- Rewrite the page tests against the new behaviour.

**Non-Goals:**

- No backend change. `AdminAccountDto` and `GET /api/v1/admin/accounts` already supply everything.
- No ESI/orphan-character lookup on this page — that stays on `/admin/blocks`.
- No new datagrid dependency (TanStack-style column resize/pin/virtual-scroll are out of scope for a roster of account-holders).
- No new admin actions (block, set-main, etc.) on this surface — read-only triage view, as today.

## Decisions

### Row unit = account, labelled by main character

One row per `AdminAccountDto`. The account has no human label of its own (just a UUID), so the row is labelled by the main character's name: the character with `is_main === true`, else the first character by name (`[...characters].sort(by name)[0]`). Alts collapse to a `+N` count; expansion reveals the full per-character table.

*Alternative — row per character (flat):* rejected. The admin's mental model is "an account/pilot," token transfers and expiries are reasoned about per-account, and a flat character list fragments that. Account rows with an Issues roll-up put triage signal where it belongs.

### Pure client-side over `data.accounts`; drop the search action

The grid filters/sorts `data.accounts` in the browser via `$derived`. The `?/search` action, `searchCharacters` import, and `CharacterSearchResultDto` usage are removed from this page (they remain in `$lib/api.ts` for `/admin/blocks`). `+page.server.ts` keeps only the `listAdminAccounts` load.

*Rationale:* the data is already loaded; a round-trip per keystroke would be strictly worse, and the orphan-lookup capability the action provided is explicitly not wanted here.

### Hand-rolled grid mechanics (no library)

- **Text filter:** `$state` string; a `$derived` predicate matches the main name OR any alt name (case-insensitive substring), so filtering by an alt surfaces its account row.
- **Status filter:** `$state` of `'all' | 'problems' | 'expired' | 'transferred'`; account-level — an account passes if any character matches (`problems` = any `token_status !== 'active'`; `expired`/`transferred` = any character in that state). Rendered as chips, mirroring the existing chip pattern in the current modal.
- **Sort:** `$state` `{ column, dir }`; `$derived` sorted list. Columns: Account (main name), Status, Admin, Issues severity (worst token state then problem count), Created. Click header to toggle asc/desc.
- **Expand:** `$state` `Set<accountId>` (or a single open id) toggled by the row's `▸`/`▾` control; expanded rows render the per-character token table — reused verbatim from today's modal (token-status dots/colours, `tokenLabel()`, main badge).

*Rationale:* the dataset is small (server account-holders) and `sveltekit-node` bans heavy deps; `$derived` makes sort/filter trivial and instant. Raising a skill exception for a grid library is unwarranted here.

### Issues roll-up column

Per account, compute counts of `token_expired` and `owner_mismatch` characters; render as e.g. `● N expired` / `▲ N transferred`, or `—` when all active. This is the at-a-glance triage signal and also the sort key for the Issues column. Reuses the existing token-state colour tokens (red = expired, amber = owner_mismatch, emerald = active).

### i18n — remove dead, add new, keep tight

Remove `admin_characters_search_*`, `admin_characters_inspect`, `admin_characters_search_empty`, `admin_characters_search_orphan`, `admin_characters_dialog_*`, and `admin_characters_filter_*` if no longer used. Add grid strings (column headers, filter chips, `+N alts`, issue counts) across the synced locale set (`en`/`de`/`fr`). Keep `admin_characters_token_*` labels (still used by the per-character table). Run Paraglide compile from `frontend/` (per project memory, not via `--filter`).

### Tests rewritten as first-class

- `page.svelte.test.ts`: grid renders a row per account labelled by main (and first-char fallback when no main); Issues roll-up appears unexpanded; expand/collapse reveals/hides the per-character table; text filter matches main and alt names; status chips filter at account level; column sort toggles order.
- `page.server.test.ts`: load returns `accounts` from `listAdminAccounts` (cookie forwarded); the `?/search` action coverage is deleted.

## Risks / Trade-offs

- **Wide rows for large accounts** → the `+N` summary + expand keeps the collapsed row compact; the per-character detail only appears on expand. No inline alt-chip overflow.
- **Account with no main** → defined fallback (first character by name); covered by an explicit unit test so the label never renders blank.
- **Removing i18n keys still referenced elsewhere** → before deleting, grep the catalogue/usages; only `admin_characters_*` keys exclusive to search/modal are removed. `svelte-check` (Paraglide compile) will fail loudly on any dangling reference, caught by the required gate.
- **e2e was wired to the search/modal flow** → Playwright specs touching `/admin/characters` must be updated to the grid (filter/expand selectors). The full three-command gate (vitest + `check` + e2e) per CLAUDE.md guards the destructive-wiring/route regressions this rewrite could introduce.
