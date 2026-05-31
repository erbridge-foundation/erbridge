# Tasks

## 1. ESI layer: authenticated character search + token use

- [ ] 1.1 Add an authenticated-ESI helper (e.g. `backend/src/esi/search.rs`) with `character_search(http, admin_character_id, access_token, q, limit) -> Result<Vec<i64>>`: calls `GET {ESI_BASE}/characters/{admin_character_id}/search/?categories=character&search=<q>&strict=false` with `Authorization: Bearer <access_token>` and the `X-Compatibility-Date` header (a documented date constant). Returns the `character` ID array. Caller guarantees `q` ≥ 3 chars. Map ESI 403 / non-2xx / network error to a typed error the service can classify as "unavailable".
- [ ] 1.2 ID→name resolution: reuse/extend `esi/public_info.rs` to resolve a batch of character IDs to `(eve_character_id, name)` (name from public-info), capped at the result maximum. Best-effort per id; drop ids that fail to resolve.
- [ ] 1.3 Token access + best-effort refresh helper (first authenticated-ESI token use in the codebase): decrypt a character's stored access token; if expired, attempt a refresh-grant via the stored refresh token, re-encrypt and persist the new tokens (reuse the existing encryption + token-column machinery). Return the usable access token, or a typed "no usable token" outcome (no refresh token / refresh rejected). Token is only ever held transiently.
- [ ] 1.4 Unit tests: search builds the correct URL + headers; 403/non-2xx/network → unavailable classification; id-resolution drops unresolvable ids + caps; token helper expired→refresh→token, and unrefreshable→no-usable-token.

## 2. Service layer: esi_search_characters orchestration

- [ ] 2.1 Add `services/admin.rs::esi_search_characters(pool, http, admin_account_id, q, limit) -> EsiSearchOutcome` where `EsiSearchOutcome` is `Available(Vec<EsiCharacterSearchResult>)` | `Unavailable(reason)`. Resolve the admin's main character; if it has no usable token (via §1.3) → `Unavailable`. Otherwise ESI-search (§1.1) → resolve names (§1.2) → annotate each with `portrait_url` (deterministic) and `already_blocked` (block-list lookup). ESI/token failure → `Unavailable(reason)`; a completed search with no hits → `Available(vec![])`. Service imports no HTTP types.
- [ ] 2.2 Enrich the existing local-DB `search_characters` result path so its results also carry `portrait_url` + `already_blocked` (extend `db::characters::search_by_name` only if needed to surface what the annotation requires; prefer annotating in the service via a block-list lookup over a wider join). Keep substring/case-insensitive/cap behaviour unchanged.
- [ ] 2.3 Unit tests (sqlx + mocked/abstracted ESI boundary): `Available` with annotated results; `Unavailable` when the main has no usable token; `Unavailable` on ESI failure; empty-but-available vs unavailable distinction; `already_blocked` true/false annotation; local search now returns `portrait_url` + `already_blocked`.

## 3. Handler + DTO + routing

- [ ] 3.1 Add `GET /api/v1/admin/characters/esi-search` handler in `handlers/api/v1/admin.rs`, taking `AdminAccount`. Validate `q` ≥ 3 chars in the handler → 400 below 3 (before any token/ESI work). Call the service; map `Available` → `200 { data: [...], unavailable: false }`, `Unavailable(reason)` → `200 { data: [], unavailable: true, reason }`. Never 5xx for a degraded ESI/token; never leak the token.
- [ ] 3.2 DTOs in `dto/admin.rs`: add `portrait_url` + `already_blocked` to the local character-search result DTO; add `EsiCharacterSearchResultDto` and a page/wrapper DTO carrying the `unavailable` indicator + reason. `From<DomainModel>` impls; no DB/domain model serialised directly.
- [ ] 3.3 Register the route in `lib.rs` (nested under `/api/v1/admin`) AND add it to `registered_admin_routes()` so the fail-closed admin-coverage test gates it (401/403). Add `#[utoipa::path]` + schemas + the `admin` tag entry in `openapi.rs`.
- [ ] 3.4 Integration tests (`tests/admin.rs` or a sibling): esi-search happy path (mock/stub the ESI boundary so the test is hermetic) returning annotated results; `q` < 3 → 400; token-unavailable → `200` empty + `unavailable`; ESI-down → `200` empty + `unavailable`; `already_blocked` annotation; admin-gating (no-credential 401, non-admin 403, bearer-key 401). Confirm the local `search` endpoint now returns `portrait_url` + `already_blocked`.

## 4. HURL coverage

- [ ] 4.1 Extend `tests/hurl/admin.hurl` (or add `esi_search.hurl`): unauthenticated → 401; non-admin → 403; bearer-key → 401; `q` too short → 400; admin-session happy/unavailable documented (the live ESI call is operator-run like the other cookie-session flows; the no-credential + 400 prefix runs hermetically). Document prerequisite cookie vars in the header + README.

## 5. Frontend: block-by-search picker

- [ ] 5.1 Rework `frontend/src/routes/admin/blocks/+page.server.ts`: drop the raw-ID `block` action's ID field path. Add a `search` action (local DB) and an `esiSearch` action (ESI fallback) that forward the cookie and call the new endpoints; both enforce `q` ≥ 3. The `block` action takes a resolved `eve_character_id` (+ optional reason) from the picker, not a free-typed ID. Add a `corpLookup` affordance (action or `+server.ts`) that fetches the selected character's corporation for the confirmation. Surface the `unavailable` indicator from esi-search.
- [ ] 5.2 Rework `frontend/src/routes/admin/blocks/+page.svelte`: replace the EVE-character-ID number input with a name search (min 3 chars). Show local results first; when empty, offer "search ESI". Render each result with portrait + name, mark `already_blocked`. On select, fetch corp and open a `ConfirmDialog` ("Block <name> of <corp>?") → submit block with the resolved id. Show the "ESI search unavailable — re-authorise your character" notice when the indicator is set. Keep the existing block list + unblock flow.
- [ ] 5.3 Update `frontend/src/lib/api.ts`: add `searchCharactersEsi(...)`; extend the local-search + result types with `portrait_url` + `already_blocked`; add a corp-lookup call if not reusing an existing one. All forward `cookie`.
- [ ] 5.4 i18n: add message keys for all new block-search UI copy (search placeholder, "search ESI", min-3 hint, unavailable notice, confirm "Block <name> of <corp>?", already-blocked marker) to BOTH `messages/en.json` and `messages/de.json`; compile paraglide (`pnpm run paraglide` from inside `frontend/`).
- [ ] 5.5 Frontend unit/component tests (Vitest): block-search action min-length validation; DB-first then ESI-fallback path; `unavailable` indicator surfaced; selecting a result resolves the right `eve_character_id`; corp shown in the confirm; raw-ID field absent.
- [ ] 5.6 Playwright e2e: extend `tests/e2e/admin.spec.ts` + `mock-backend.ts` — local search → select → confirm (with corp) → block; ESI-fallback search → block a never-seen id; ESI-unavailable notice renders; no raw-ID field present.

## 6. Drift + tidy

- [ ] 6.1 `cargo sqlx prepare -- --all-targets` from `backend/`; commit the regenerated `.sqlx/` cache.
- [ ] 6.2 Confirm no unintended change to the existing `POST /api/v1/admin/blocks` wire contract (still `{ eve_character_id, reason }`).

## 7. Verification

### Backend

- [ ] 7.1 `cargo fmt --check` from `backend/`.
- [ ] 7.2 `cargo clippy --all-targets --all-features -- -D warnings` from `backend/`.
- [ ] 7.3 `cargo sqlx prepare --check -- --all-targets` from `backend/`.
- [ ] 7.4 `cargo test --all-targets` from `backend/` — all unit + integration tests pass, including the admin-coverage test now covering esi-search, the 3-char guard, and the graceful-unavailable paths.
- [ ] 7.5 Hurl run against the live dev stack: the no-credential + 400 prefix hermetically; the admin-session ESI flow operator-run.

### Frontend (all three are required by project policy — `pnpm test` alone is NOT sufficient)

- [ ] 7.6 `pnpm --filter frontend test` — Vitest unit/component tests.
- [ ] 7.7 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile).
- [ ] 7.8 `pnpm --filter frontend run test:e2e` — Playwright e2e tests.

## 8. Wrap-up

- [ ] 8.1 `openspec validate add-block-character-search --strict` — must pass.
- [ ] 8.2 Update memory: note the block-by-search redesign + the first authenticated-ESI-call/token-refresh capability; cross-link `project-frontend-status`, `project-admin-block-change`, `project-backend-auth-model`.
