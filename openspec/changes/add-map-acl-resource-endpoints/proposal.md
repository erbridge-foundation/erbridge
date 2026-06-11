# Add Map/ACL Resource Endpoints

## Why

A frontend review (2026-06-11) found two costs of the missing single-resource reads: the ACL detail and map settings pages fetch the caller's *entire* list and `find()` the one entity (documented workarounds in both load functions), and — more seriously — the "create map with default ACL" flow orchestrates create-ACL → seed-member → create-map as three separate backend calls from the frontend. When `createMap` fails (the slug-conflict 409 being the likely failure), the freshly created ACL is stranded, and every retry mints another orphan ACL named after the map — in a system that deliberately has no orphan-ACL reaping.

## What Changes

- `GET /api/v1/acls/{acl_id}` — single ACL the caller can manage (404 when absent *or* not manageable, matching the list's visibility).
- `GET /api/v1/maps/by-slug/{slug}` — single map the caller can read, resolved by slug (the frontend's natural key for `/maps/[slug]` routes), including its attached-ACL summaries like the list.
- `POST /api/v1/maps` gains `default_acl: bool` — when true, the backend creates an ACL named after the map, seeds the caller's main character as an explicit `admin` member (when a main exists), attaches it, and creates the map, all in one transaction. The `acl_id` attach-existing parameter remains.
- Frontend: `acls/[id]` and `maps/[slug]`/`maps/[slug]/settings` loads switch to the single-resource endpoints; the create-map action sends `default_acl: true` instead of orchestrating, eliminating the leak.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `acls`: gains the single-ACL read requirement.
- `maps`: gains the by-slug read requirement; map creation gains the atomic default-ACL option.

## Impact

- Backend: `handlers/api/v1/acls.rs` + `maps.rs` (new routes), `services/acl.rs` + `map.rs`, `db/acl.rs` + `map.rs` (single-row queries), `lib.rs` route table + doc-coverage lists, OpenAPI.
- Frontend: three load functions simplified; `maps/+page.server.ts` create action shrinks; `src/lib/api.ts` gains `getAcl`/`getMapBySlug` and the `default_acl` field.
- Audit: the default-ACL path emits the existing `acl_created`, `acl_member_added`, `acl_attached_to_map`, `map_created` events in one transaction — no new event types.
- Tests: integration + HURL for both new endpoints and the default-ACL creation; frontend Vitest/e2e updates.
