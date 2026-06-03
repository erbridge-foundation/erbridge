## 1. ESI search generalization (`esi-search` capability)

- [x] 1.1 In `backend/src/esi/search.rs`, generalize `character_search` to accept a set of categories (`character`, `corporation`, `alliance`) and build the `categories=<comma-separated>` query param; deserialize the ESI response as an object with per-category id arrays. Preserve the `EsiSearchError { Rejected, Unavailable }` outcome and the `X-Compatibility-Date` + bearer-auth headers.
- [x] 1.2 Add corporation/alliance name resolution, reusing `esi::public_info::fetch_corporation_name` / `fetch_alliance_name`; keep `resolve_character_names` for characters. Drop ids whose name lookup fails; cap results per category.
- [x] 1.3 Update / add unit tests in `esi/search.rs` (wiremock): multi-category response parsing, corp/alliance name resolution, unresolvable-id drop, the existing 403→`Rejected` and unreachable→`Unavailable` cases, and the too-short-fragment guard.

## 2. Orphan-character find-or-mint (`data-persistence` capability)

- [x] 2.1 In `backend/src/db/characters.rs`, add a `find_id_by_eve_character_id` lookup and a `mint_orphan` insert (`account_id = NULL`, NULL tokens, empty `scopes`, `is_main = false`, public-info columns populated). Reuse the existing insert where practical; no migration (the schema already allows nullable `account_id`).
- [x] 2.2 Add `sqlx::test` integration tests: mint creates an orphan with the specified NULL/empty columns; find returns an existing row's `id` (account-owned and orphan) without inserting; the minted `id` is referenceable as an `acl_member.character_id`.

## 3. Entity-search service (`entity-search` capability)

- [x] 3.1 Create `backend/src/services/entity_search.rs`: orchestrate token acquisition for the requesting account (extract/share the admin path's usable-main-token logic from `services/admin.rs`), call the multi-category ESI search, resolve character matches to `eve_character.id` UUIDs via find-or-mint (§2), and return results grouped by category with the per-type identifier. Map no-usable-token / 403 / unreachable to the graceful "unavailable" outcome.
- [x] 3.2 Refactor `services/admin.rs::esi_search_characters` to delegate to the new service (character category only), preserving its existing response contract.
- [x] 3.3 Unit tests for the service: available-with-matches, available-empty, unavailable (no token), and the character→UUID resolution / orphan-mint path (wiremock ESI + test pool).

## 4. DTOs, response envelope, and handler (`entity-search` capability)

- [x] 4.1 Add request/response DTOs in `backend/src/dto/` for the unified search result (per-category result lists; character carries `eve_character.id` UUID + name; corp/alliance carry `eve_entity_id` + name; plus the available/unavailable signal).
- [x] 4.2 Add the OpenAPI response alias in `backend/src/response.rs` (mirroring the existing `EsiCharacterSearchResponse` pattern).
- [x] 4.3 Create `backend/src/handlers/api/v1/entities.rs` with `GET /api/v1/entities/search`: `AuthenticatedAccount` extractor (account-auth, NOT admin-only), parse `q` + optional `categories`, enforce `MIN_SEARCH_LEN`, call the service, return the `{ "data": ... }` envelope; errors as `{ "error": { code, message } }`.
- [x] 4.4 Wire the route in `backend/src/lib.rs` and register the handler + DTOs in `backend/src/openapi.rs`.

## 5. Tests & verification

- [x] 5.1 Add `backend/tests/hurl/entities.hurl`: auth required (401 when unauthenticated), too-short `q` rejected, a successful multi-category search, a `categories` filter, and the orphan-mint-then-referenceable flow against the live/mock backend. Follow the existing `characters.hurl` / `acls.hurl` conventions.
- [x] 5.2 Confirm the admin character-search integration/HURL coverage still passes unchanged after the delegation refactor.
- [x] 5.3 Run the full backend verification: `cargo test` (unit + integration) green, `cargo clippy` clean, `cargo sqlx prepare`/offline check shows no drift, and the live HURL suites pass (including the new `entities.hurl`).

## 6. OpenSpec hygiene

- [x] 6.1 Run `openspec validate add-entity-search --strict` and resolve any issues.
- [x] 6.2 Update memory `project-maps-acls.md` to note `add-entity-search` status and that `add-maps-and-acls-ui` depends on the `GET /api/v1/entities/search` endpoint + ghost/orphan-on-select behavior.
