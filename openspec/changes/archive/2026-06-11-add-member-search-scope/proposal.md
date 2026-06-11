## Why

The ACL member picker's entity search always queries all three ESI categories (character, corporation, alliance) at once. When the user already knows what kind of member they're adding, that fans out into unnecessary outbound ESI work and slower, noisier results. Letting the user scope the search to a single category makes the call quicker and the result list focused.

This was implemented and shipped on `develop` (commit `1b95d69`) on top of the already-archived `add-maps-and-acls-ui`; this change records the new requirement faithfully rather than retro-editing the archived change.

## What Changes

- Add a **scope radio group** to the member picker with options `character`, `corporation`, `alliance`, and `any` (default `any`), submitted inside the existing `?/search` form.
- The `/acls/[id]` `search` form action maps the chosen scope to the backend `categories` query parameter: a single category passes through; `any` (or absent/unrecognized) omits `categories` so the backend applies its all-three default.
- No backend change: `GET /api/v1/entities/search` already accepts `categories`, and the `searchEntities(...)` client function already forwards an optional `categories` argument.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `maps-acls-ui`: the **Entity-search member picker** requirement gains the ability to scope the search to a single ESI category (or all) via a picker control, narrowing the `GET /api/v1/entities/search` `categories` parameter.

## Impact

- Frontend only:
  - `frontend/src/lib/components/MemberPicker.svelte` — scope radio group in the search form.
  - `frontend/src/routes/acls/[id]/+page.server.ts` — `search` action maps `scope` → `categories`.
  - `frontend/messages/{en,de,fr}.json` — `picker_scope_*` strings.
- No backend, API-contract, or dependency changes.
- Tests: MemberPicker component tests (radios render, default `any`, narrowing) and `acls/[id]` server-action tests (scope→categories mapping).
