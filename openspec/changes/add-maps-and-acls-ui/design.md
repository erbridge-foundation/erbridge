## Context

The backend exposes 15 maps/ACL endpoints (`add-maps-and-acls`, archived) plus the account-authenticated `GET /api/v1/entities/search` (`add-entity-search`, archived). The frontend has nothing for them: `src/routes/maps/+page.svelte` is a placeholder and there is no ACL route. The established frontend conventions (from the admin UI and block-picker work) are the constraints here:

- `src/lib/api.ts`: one typed function per endpoint, signature `(fetch, backendUrl, ...args, cookie)`, going through the `request<T>()` helper that unwraps the `{ data }` envelope and throws `ApiError(code, message, status)` on non-2xx. The internal backend URL comes from `backend_internal_url()`; the cookie is forwarded from `request.headers.get('cookie')`.
- Data via `+page.server.ts` `load`; mutations via **form actions** returning `fail(status, { action, code, message })`, never raw `fetch` from components. Forms use `use:enhance`.
- Native CSS with the design tokens in `src/app.css`; no Tailwind, no grid library.
- i18n via paraglide; strings live in `messages/{en,de,fr}.json` and MUST stay in sync across all three (the locale set is enforced).
- Svelte 5 runes only.

The relevant DTO shapes (already in the backend): `MapDto { id, name, slug, owner_account_id, description, acls: AclSummaryDto[], created_at, updated_at }` with `AclSummaryDto { id, name }`; `AclDto { id, name, owner_account_id, ... }` (no attachment count); `AclMemberDto { id, acl_id, member_type, eve_entity_id?, character_id?, name, permission, ... }`; `EntitySearchPageDto { characters: {id, eve_character_id, name}[], corporations: {eve_entity_id, name}[], alliances: {eve_entity_id, name}[], unavailable }`.

## Goals / Non-Goals

**Goals:**
- A complete account-facing management surface for maps and ACLs over the existing endpoints, following the established `api.ts` + load/actions + native-CSS + paraglide conventions exactly.
- A member picker that resolves a typed name to the identifier ACL membership needs, reusing the entity-search endpoint and surfacing its unavailable-vs-empty distinction.
- Full test coverage per the project rule: Vitest, `svelte-check`, and Playwright all green.

**Non-Goals:**
- Map drawing / canvas / Svelte Flow — map *contents* are explicitly out of scope; this is map *containers* + access control only.
- An "unattached ACL" indicator (dropped — see Decisions).
- Any backend or schema change; any new client dependency.
- Server-admin tooling — this is the account-owner surface, not the admin tier.

## Decisions

### Decision: A new `maps-acls-ui` capability, not a modification of `frontend-patterns`

The behavior here is a discrete user-facing surface, so it gets its own capability spec (mirroring how the admin UI changes carried their own frontend capability) rather than amending the generic `frontend-patterns` spec. The backend `maps`/`acls`/`entity-search` capabilities already own the API contract; this capability owns only the frontend behavior (routes, what each surface lets a user do, the picker, the client-side gating).

### Decision: Maps = simple list + inline forms; ACLs = master/detail

- **Maps** (`/maps`): a flat list with inline create and per-row edit/delete. Few enough fields (name, slug, description) that a datagrid adds nothing.
- **ACLs** (`/acls` → `/acls/[id]`): a list that links to a detail route where members are managed. Members are a second resource under an ACL, so master/detail is the natural shape.
- Neither needs a grid library — **Alternative considered**: a datagrid component (as used for admin characters). Rejected: the row counts and column counts here are small, and inline/master-detail is simpler and matches the data.

### Decision: Map detail is slug-keyed (`/maps/[slug]`); ACL detail is UUID-keyed (`/acls/[id]`)

A map has a human-meaningful, globally-unique `slug`, so the address bar shows `/maps/the-slug`. The detail `load` resolves the slug by finding it in the account's `GET /maps` list (which already embeds each map's ACLs) rather than adding a backend slug-lookup endpoint — **Alternative considered**: a `GET /maps/by-slug/{slug}` endpoint. Rejected: no backend change is in scope and the list is already loaded/cheap. ACLs have no slug, so `/acls/[id]` stays UUID-keyed.

- Edge case: a slug not present in the list → the detail `load` throws `error(404)` (SvelteKit not-found), consistent with "you can't see it / it doesn't exist."

### Decision: The member picker reuses `GET /api/v1/entities/search` via a form action

The picker is a shared component fed by a `search` form action that calls the entity-search client function. The user types ≥3 chars, the action returns results grouped by category (or `unavailable: true`), the user selects one, and the **selection carries the already-resolved identifier** straight into the `addMember` action — character selections send `character_id` (the `eve_character.id` UUID the search minted/returned), corp/alliance selections send `eve_entity_id`. This mirrors the existing block-picker's search→select→act flow, so there is one familiar pattern.

- The search action enforces the 3-char minimum before the round-trip and maps `unavailable` to a distinct UI state ("search unavailable", not "no matches"), exactly as the block ESI picker does.

### Decision: Client-side gate `manage`/`admin` to character members

The permission `<select>` offered when adding/editing a member is conditioned on the chosen `member_type`: `manage` and `admin` appear only for `character`. This is a UX guard; the backend CHECK constraint remains the authority, and a rejected add surfaces as a `fail()` with the backend's error code.

### Decision: Drop the "unattached ACL" indicator

`GET /acls` carries no attachment count, and the only client-side approximation (cross-referencing every map's embedded `acls`) is both expensive and incomplete (it can't see maps the account can't read). The indicator was originally meant to replace the dropped ACL-reaper; with the reaper gone too, it has no consumer worth the cost. Dropped.

### Decision: Mutations are form actions with the `{ action, code, message }` fail shape

Every create/rename/delete/attach/detach/add-member/update-member/remove-member is a named form action returning `fail(status, { action, code, message })` on error (the established shape, with `action` discriminating which form failed on a multi-form page). `use:enhance` everywhere. Successful mutations rely on SvelteKit's automatic `load` re-run to refresh the list/detail.

## Risks / Trade-offs

- **Slug-from-list resolution can go stale** → if a map's slug changed in another tab, the detail link 404s until the list reloads. Mitigation: acceptable — `load` re-runs on navigation; a 404 is the correct "not found under that slug" answer.
- **Member picker mints orphans on the backend for searched-but-unselected characters** → already accepted in `add-entity-search` (orphans are cheap, token-less, self-healing). No new frontend mitigation needed.
- **Client-side permission gating can drift from the backend CHECK** → mitigation: the backend is the authority; the UI guard is convenience only, and a violation still surfaces as a handled `fail()`.
- **i18n drift across en/de/fr** → mitigation: add every new key to all three locale files in the same change; `svelte-check` (paraglide compile) catches a missing key.
- **e2e against live entity search is flaky/slow** → mitigation: follow the block-picker e2e approach — drive the picker against the mock backend / network-boundary fulfilment rather than real ESI.

## Post-implementation revisions (2026-06-04, user feedback)

- **Create is a dialog, not an inline form.** A "create map" button opens a modal (new generic `Modal.svelte`, distinct from the confirm-only `ConfirmDialog`) holding the name/slug/description (textarea) form plus a "create a default ACL" checkbox.
- **Default ACL on create.** With the checkbox on, the create action creates a reusable ACL named after the map, adds the account's main character as an explicit `admin` member (skipped — empty ACL — if no main; best-effort if the add fails), then creates the map attached to it. Two extra client calls (`createAcl`, `addAclMember` + a `getMe` to find the main); no backend change.
- **Nav model corrected.** The map *name* now links to `/maps/[slug]` which is the **canvas** (placeholder until built); the *edit* control links to the new `/maps/[slug]/settings` route which holds the edit form + ACL attach/detach. The earlier design had the name → a detail page and an inline 3-textbox edit; both are replaced.
- **Canvas has no chrome bar.** The canvas placeholder fills the whole area with no header strip (the wireframe's tab-bar is dropped for now). The current map's name is carried only by the browser tab title; map settings are reached by going back to the `/maps` list and using its edit control. No in-canvas name/settings affordance.
- **Picker portraits.** Search results show a portrait/logo derived from the EVE public image CDN (`images.evetech.net`) by id — no backend change.
- **CEO tooltip deferred.** A corp→CEO tooltip needs data the search endpoint does not carry (an extra ESI lookup); captured as future work rather than expanding this frontend change.

## Post-implementation revisions (2026-06-04, second feedback round)

- **Picker add is inline per result.** The earlier "select a result → a permission box appears at the bottom → add" flow is replaced: each result row now carries its own permission `<select>` (gated to the member type) and an inline "add" button. The `MemberPicker` no longer takes an `onSelect` callback or holds a `selected` member; it renders an `?/addMember` form per result. The ACL detail page's separate add-form and `selected`/`permission` state are gone.
- **Active-search indicator + Enter-to-search.** The search field shows a spinner and a lit border while the `?/search` request is in flight (tracked via `use:enhance`), and the native `<form>` submits on Enter so the account need not click the search button.
- **`/acls` create is a dialog.** Matching the maps create flow, `/acls` now opens a `Modal` from a "create ACL" button instead of an always-present inline create form.
- **Maps list drops the "no ACLs attached" text.** The ACL summary renders only when the viewer can see attached ACLs; an empty list shows nothing (it does not mean "none" — the viewer may lack manage permission). Removed the `maps_no_acls` string.

## Open Questions

- Should the map "create" form let you attach an ACL at creation (the `CreateMapRequest.acl_id` is optional) or always attach from the detail page afterward? Leaning: offer it on the detail page for a single clear attach flow, and treat create-time attach as a possible later enhancement — to be settled when building the form.
- Where should `/acls` live in the global nav (top-level, or under a "maps" grouping)? Leaning top-level alongside `/maps`, but defer to the nav's existing structure when wiring it.
