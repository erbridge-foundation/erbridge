# Tasks — add-map-acl-resource-endpoints

## 1. Single-resource reads

- [x] 1.1 `db/acl.rs`: single-ACL query under the manageable predicate (extend the manageable-list query with an id filter rather than a second predicate copy); `services/acl::get_manageable`; handler + route `GET /api/v1/acls/{acl_id}`
- [x] 1.2 `db/map.rs`: by-slug lookup; `services/map::get_map_by_slug` reusing the read-permission check; handler + route `GET /api/v1/maps/by-slug/{slug}`
- [x] 1.3 Register both in `lib.rs` (router + `registered_api_v1_routes`), add OpenAPI paths/schemas
- [x] 1.4 Integration tests per visibility scenario (owner/manager/unrelated/unknown; reader/unreadable/soft-deleted); HURL coverage in `acls.hurl` / `maps.hurl`

## 2. Atomic default-ACL creation

- [x] 2.1 Add `default_acl: Option<bool>` to the create-map DTO; reject `default_acl && acl_id` with 400
- [x] 2.2 `services/map::create_map`: when set, create ACL (name = map name) → seed main as `admin` member when present → insert map → attach → four audit events, one transaction
- [x] 2.3 Integration tests: all-or-nothing on slug conflict (no stray ACL), seeded vs no-main variants, mutual-exclusion 400; HURL for the happy path

## 3. Frontend adoption

- [x] 3.1 `src/lib/api.ts`: `getAcl`, `getMapBySlug`, `CreateMapRequest.default_acl`; remove now-unneeded orchestration imports
- [x] 3.2 Switch `acls/[id]`, `maps/[slug]`, `maps/[slug]/settings` loads to the single-resource endpoints (delete the list-then-find paths); create action sends `default_acl: true` and drops the createAcl/getMe/addAclMember orchestration
- [x] 3.3 Update Vitest suites for the three loads + create action; update the e2e mock backend with the new endpoints

## 4. Verification

- [x] 4.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`; `cargo sqlx prepare -- --all-targets` and commit the cache diff
- [x] 4.2 `pnpm test` (run from `frontend/`) — Vitest unit/component tests (310 passed)
- [x] 4.3 `pnpm run check` (run from `frontend/`) — svelte-check (0 errors, 0 warnings)
- [x] 4.4 `pnpm run test:e2e` (run from `frontend/`) — Playwright e2e tests (29 passed)
- [x] 4.5 Live smoke test on dev compose: create a map with default ACL, force a slug conflict and confirm no stray ACL appears in `/acls`
      - Backend image rebuilt + restarted on the dev stack (port 5000); `/api/health` ok, both new OpenAPI paths present, `/maps/by-slug/{slug}` rejects anon with 401.
      - Live smoke PASSED: default-ACL create seeds the main as admin; slug-conflict 409 leaves no stray ACL.
