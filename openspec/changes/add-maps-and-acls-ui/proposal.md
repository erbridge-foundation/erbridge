## Why

The maps + ACLs backend (`add-maps-and-acls`) and the entity-search backend (`add-entity-search`) have shipped, but the frontend has none of it: `/maps` is a "coming soon" placeholder and there is no ACL UI at all. Account holders cannot create a map, manage who can read or edit it, or build the reusable access-control lists the resolver depends on. This change builds the management surface that turns those 16 endpoints into something a user can actually operate.

## What Changes

- Replace the `/maps` placeholder with a **maps management** surface: list the maps the account can see, create a map (name + slug + optional description, optional ACL attached at creation), rename/edit, and soft-delete. Map drawing/canvas remains **out of scope**.
- Add a **map detail** route keyed by slug (`/maps/[slug]`, slug resolved from the maps list — no backend slug lookup) showing the map's attached ACLs with **attach** (pick from the account's manageable ACLs) and **detach** controls.
- Add an **ACLs management** surface (`/acls`): list the ACLs the account can manage, create/rename/delete, and a master/detail **ACL detail** route keyed by UUID (`/acls/[id]`) for member management — add member, change a member's permission, remove a member, for `character`/`corporation`/`alliance` member types.
- The **member picker** uses the account-authenticated entity search (`GET /api/v1/entities/search`): the user types a name, picks a result, and the picker sends the resolved identifier (character → `eve_character.id` UUID, corp/alliance → numeric `eve_entity_id`) to the add-member action. The search "unavailable" outcome is surfaced distinctly from "no matches".
- Client-side enforce the backend's member-type/permission rule: `manage` and `admin` are offered only for `character` members (the backend CHECK constraint is the backstop).
- Typed client functions in `$lib/api.ts` (one per endpoint, cookie-forwarded, throwing `ApiError`), SvelteKit `load` + form actions returning `fail(status, { action, code, message })`, native CSS with design tokens, and paraglide i18n strings added to all three locales (en/de/fr).

The **"unattached ACL" indicator is dropped** (decided during explore): `GET /acls` carries no attachment count, so it is not cheaply derivable and not worth the round-trips.

This change is **frontend-only**. It consumes the existing `/api/v1/{maps,acls,entities}` endpoints; no backend or schema changes.

## Capabilities

### New Capabilities

- `maps-acls-ui`: The account-facing SvelteKit management surface for maps and ACLs — maps list/create/edit/soft-delete, the slug-keyed map detail with ACL attach/detach, ACL list/create/rename/delete, the UUID-keyed ACL detail with member CRUD, and the entity-search-backed member picker (with client-side member-type/permission gating and the unavailable-vs-empty distinction). Covers the routes, load functions, form actions, and the `$lib/api.ts` client functions for these endpoints.

### Modified Capabilities

<!-- None. The backend maps/acls/entity-search capabilities already specify the API contract; this change adds only frontend behavior, captured in the new maps-acls-ui capability. -->

## Impact

- **Frontend code**:
  - `src/lib/api.ts` — new typed DTOs + client functions for `GET/POST /acls`, `PATCH/DELETE /acls/{id}`, `GET/POST /acls/{id}/members`, `PATCH/DELETE /acls/{id}/members/{member_id}`, `GET/POST /maps`, `GET/PATCH/DELETE /maps/{id}`, `POST /maps/{id}/acls`, `DELETE /maps/{id}/acls/{acl_id}`, and `GET /entities/search`.
  - `src/routes/maps/` — replace `+page.svelte` placeholder; add `+page.server.ts` (load + create/edit/delete actions) and a `[slug]/` detail route (load resolves slug from the list; attach/detach actions).
  - `src/routes/acls/` — new `+page.svelte` + `+page.server.ts` (list + create/rename/delete) and `[id]/` detail route (`+page.svelte` + `+page.server.ts`: members load + add/update/remove + the entity-search picker action).
  - `src/lib/components/` — shared member-picker component (entity search → select → resolved identifier) and any small dialog/list components, consistent with the existing block-picker pattern.
  - `messages/{en,de,fr}.json` — strings for maps/ACLs/member-picker UI (kept in sync across all three locales).
- **APIs**: none new — consumes existing `/api/v1/{maps,acls,entities}` endpoints.
- **Schema**: none.
- **Tests**: Vitest unit/component coverage (load functions, form actions, picker component logic, `api.ts` functions), `svelte-check`, and Playwright e2e for the maps and ACL management flows (create→attach→detach, member add→update→remove, destructive-action wiring).
- **Dependencies**: none new (no grid library — maps is a simple list + inline forms, ACLs is master/detail).
