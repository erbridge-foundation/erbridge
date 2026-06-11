## 1. API client (`$lib/api.ts`)

- [x] 1.1 Add TypeScript DTOs mirroring the backend: `AclSummaryDto`, `MapDto`, `CreateMapRequest`, `UpdateMapRequest`, `AclDto`, `AclMemberDto`, `AddMemberRequest`, `UpdateMemberRequest`, and entity-search `EntityCharacterDto` / `EntityOrgDto` / `EntitySearchPageDto` (each with a `// keep in sync with: backend/src/dto/<file>.rs` comment, matching the existing convention).
- [x] 1.2 Add cookie-forwarded client functions through the existing `request<T>()` helper: `listMaps`, `createMap`, `getMap`, `updateMap`, `deleteMap`, `attachAcl`, `detachAcl`; `listAcls`, `createAcl`, `renameAcl`, `deleteAcl`, `listAclMembers`, `addAclMember`, `updateAclMember`, `removeAclMember`; and `searchEntities` (with the optional `categories` query param). Follow the `(fetch, backendUrl, ...args, cookie)` signature and `ApiError` propagation already used by the admin functions.
- [x] 1.3 Co-located Vitest tests for the non-trivial client functions (URL construction incl. `searchEntities` query params, envelope unwrap, `ApiError` on non-2xx) following `src/lib/api.test.ts` conventions.

## 2. Maps surface (`/maps`)

- [x] 2.1 Replace `src/routes/maps/+page.svelte` placeholder and add `src/routes/maps/+page.server.ts`: `load` lists maps; form actions `create`, `edit`, `delete` call the client fns and return `fail(status, { action, code, message })` on `ApiError`. Forward the cookie from `request.headers`.
- [x] 2.2 Build the `/maps` page UI (native CSS + tokens): a list with each map's name/slug/attached-ACL summary, an inline create form, and per-row edit/delete, all `<form method="POST" use:enhance>`.
- [x] 2.3 Add `src/routes/maps/[slug]/+page.server.ts`: `load` resolves the slug against the maps list and `error(404)` when absent; form actions `attach` (pick a manageable ACL → `attachAcl`) and `detach` (`detachAcl`).
- [x] 2.4 Build the `/maps/[slug]` detail UI: show the map and its attached ACLs, an attach control populated from the account's manageable ACLs (`listAcls`), and per-ACL detach.

## 3. ACLs surface (`/acls`)

- [x] 3.1 Add `src/routes/acls/+page.server.ts`: `load` lists manageable ACLs; form actions `create`, `rename`, `delete` with the standard `fail` shape.
- [x] 3.2 Build the `/acls` page UI: list by name, inline create, per-row rename/delete. No "unattached" indicator.
- [x] 3.3 Add `src/routes/acls/[id]/+page.server.ts`: `load` lists the ACL's members; form actions `search` (entity-search picker), `addMember`, `updateMember`, `removeMember`. The `search` action enforces the 3-char minimum and returns the grouped results + `unavailable` flag; `addMember` reads the resolved identifier (`character_id` or `eve_entity_id`) the picker submits.
- [x] 3.4 Build the `/acls/[id]` detail UI: member list (type/name/permission), the member picker, add/edit-permission/remove controls.

## 4. Member picker component + client-side gating

- [x] 4.1 Add a shared member-picker component under `src/lib/components/` (search input → grouped results → select), mirroring the block-picker's search→select→act flow; on select it holds `{ member_type, character_id | eve_entity_id, name }` for the add-member form. Surface the `unavailable` state distinctly from empty results.
- [x] 4.2 Implement the permission gating: the permission `<select>` offers `manage`/`admin` only for `character` members; `read`/`read_write`/`deny` for all. Pure logic extracted so it is unit-testable.
- [x] 4.3 Co-located Vitest component/logic tests: picker select→identifier mapping per category, the 3-char guard, the unavailable-vs-empty rendering, and the permission-gating logic.

## 5. i18n

- [x] 5.1 Add all new UI strings (maps + ACLs + member-picker labels, errors, confirmations) to `messages/en.json`, then add the SAME keys to `messages/de.json` and `messages/fr.json` (locale set kept in sync). Run paraglide compile from `frontend/`.

## 6. Tests for load functions and actions

- [x] 6.1 Vitest tests for each `+page.server.ts` `load` (returned data shape; the `/maps/[slug]` 404 path) — mock the `$lib/api` functions.
- [x] 6.2 Vitest tests for each form action: validation branches, the `fail(status, { action, code, message })` returns on `ApiError`, and the success path (correct client fn called with the parsed args). Cover the `addMember` identifier-by-type branch and the `search` too-short guard.
- [x] 6.3 Playwright e2e specs under `frontend/tests/e2e/`: a maps flow (create → open detail → attach ACL → detach) and an ACL flow (create ACL → open detail → add member via picker → update permission → remove member), driving the entity search against the mock backend / network boundary (not real ESI), and asserting the destructive-action (delete/remove/detach) wiring.

## 7. Verification

- [x] 7.1 `pnpm --filter frontend test` — Vitest unit/component tests pass.
- [x] 7.2 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile) passes with no errors.
- [x] 7.3 `pnpm --filter frontend run test:e2e` — Playwright e2e tests pass.

## 8. OpenSpec hygiene

- [x] 8.1 Run `openspec validate add-maps-and-acls-ui --strict` and resolve any issues.
- [x] 8.2 Update memory (`project-maps-acls.md`, `project-frontend-status.md`) to record the maps/ACLs UI status once implemented.
