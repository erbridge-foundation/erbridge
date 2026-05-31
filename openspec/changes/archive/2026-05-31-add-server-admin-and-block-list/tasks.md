# Tasks

## 1. Database migration

- [x] 1.1 Create `backend/migrations/00000000000007_create_blocked_eve_character.sql` (the `00000000000006` slot was already taken by the shipped `add_audit_log_target_columns` migration, so the next free number is used): `blocked_eve_character (eve_character_id BIGINT PRIMARY KEY, character_name TEXT, corporation_name TEXT, reason TEXT, blocked_by UUID REFERENCES account(id) ON DELETE SET NULL, blocked_at TIMESTAMPTZ NOT NULL DEFAULT now())`. No FK to `eve_character`.
- [x] 1.2 Apply the migration locally and confirm the table (no `eve_character` FK) in psql.

## 2. DB layer: blocks + admin queries

- [x] 2.1 Add `backend/src/db/blocks.rs` (new resource file) with: `insert_block(tx, eve_character_id, character_name, corporation_name, reason, blocked_by) -> bool` (true if newly inserted, idempotent on conflict); `delete_block(tx, eve_character_id) -> bool`; `list_blocks(pool) -> Vec<BlockedEveCharacter>` (newest first); `is_eve_character_blocked(pool, eve_character_id) -> bool`; `account_has_blocked_character(pool, account_id) -> bool` (join `eve_character`↔`blocked_eve_character`). Wire `pub mod blocks;` into `db/mod.rs`.
- [x] 2.2 Add to `backend/src/db/accounts.rs`: `set_server_admin(tx, account_id, value) -> bool`; `account_exists(pool, account_id) -> bool`; a transactional `count_server_admins_tx(tx) -> i64` (the existing pool-based `count_server_admins` stays for the soft-delete guard). Add `list_accounts_admin(pool) -> Vec<Account>` (newest first) if a wider admin list is needed.
- [x] 2.3 Add to `backend/src/db/characters.rs`: `search_by_name(pool, fragment, limit) -> Vec<(eve_character_id, name, is_main, account_id)>` using a case-insensitive `ILIKE` bound parameter; `find_account_for_eve_character(pool, eve_character_id) -> Option<Uuid>` (the owning account, or None for orphan/unknown).
- [x] 2.4 sqlx tests for every new DB fn: insert/delete/list/idempotency for blocks; `is_eve_character_blocked` and `account_has_blocked_character` true/false; `set_server_admin` flip; `search_by_name` match + cap + injection-safety (a `%`/`_` fragment is treated literally enough to not error); `find_account_for_eve_character` for owned/orphan/unknown.

## 3. Audit: BlockedLoginRejected variant

- [x] 3.1 Add `BlockedLoginRejected { eve_character_id: i64 }` to `AuditEvent` in `backend/src/audit/mod.rs`: `event_type()` → `"blocked_login_rejected"`; `details()` → `{ "eve_character_id": … }`. (The other four admin/block variants already exist.)
- [x] 3.2 Unit test for the new variant's `event_type()` and `details()` shape.

## 4. AdminAccount extractor + coverage test

- [x] 4.1 Add `AdminAccount(pub Uuid)` extractor (in `handlers/middleware.rs` or a sibling). Session-cookie only — it SHALL NOT consult API keys. Resolve the session → account_id, load the account, require `is_server_admin = TRUE`. Reject: 401 `unauthenticated` (no session), 403 `forbidden_admin_required` (authenticated non-admin). Add `AppError::ForbiddenAdminRequired` if not already representable.
- [x] 4.2 Add `AppError::AccountBlocked` → HTTP 401 `account_blocked` in `error.rs`.
- [x] 4.3 Unit tests for the extractor: no cookie → 401; non-admin cookie → 403; admin cookie → Ok; bearer key for an admin account → 401 (keys never confer admin).
- [x] 4.4 Add an `admin_auth_coverage` integration test mirroring the existing v1 auth-coverage test: enumerate every registered `/api/v1/admin/*` route and assert each handler extracts `AdminAccount` (a route missing it fails the test). Added a `registered_admin_routes()` helper in `lib.rs` (returns empty until section 7 adds routes); the coverage test is behavioural — it asserts each registered admin route rejects no-credentials (401) and a non-admin session (403), so a handler that omits the extractor fails. Enforces vacuously now, fully as section 7 populates the helper.

## 5. Block enforcement: bearer branch + SSO callback

- [x] 5.1 In the bearer branch of `AuthenticatedAccount` (`handlers/middleware.rs`), after the existing `soft_deleted` check, add `account_has_blocked_character` → reject with `AppError::AccountBlocked`. Do NOT add any block check to the session-cookie branch.
- [x] 5.2 In `services/auth.rs::complete_sso_callback`, before any account/character write, call `db::blocks::is_eve_character_blocked`. If blocked: emit `AuditEvent::BlockedLoginRejected { eve_character_id }` (actor None, acting_as None — actor is NULL; the subject is in details) in its own short transaction (or a dedicated audit-only write), return a blocked error/outcome that the handler maps to a `/blocked` redirect. Ensure this runs for both the login and add-character paths (it precedes the `resolve_or_create` branch, so it covers both). Implemented as a new `SsoOutcome` enum (`Authenticated(Uuid)` / `Blocked`) returned by the service, with an `account_id()` accessor.
- [x] 5.3 In `handlers/auth.rs::callback`, map the blocked outcome to a redirect to `/blocked` (no cookie set, no session).
- [x] 5.4 Integration tests (`tests/blocks.rs`): blocked character login → no account/character/session written + `blocked_login_rejected` audit row (actor NULL); blocked add-character flow → not attached, audit row written; never-seen blocked id → no orphan account created; bearer request for a blocked account → 401 `account_blocked` (key not deleted); non-blocked bearer proceeds; cookie request for a (session-less) blocked account → 401 `unauthenticated` (cookie path performs no block check); non-blocked cookie request served (no block row exists, so by construction no `blocked_eve_character` query). The browser-facing `/blocked` redirect (which runs after a real ESI exchange) is covered by the section-9 Playwright e2e.

## 6. Service layer: admin operations

- [x] 6.1 Add `backend/src/services/admin.rs`. Per the existing service style (and the task's own escape hatch) it **returns `AppError` directly** rather than a separate `AdminError` enum — `services/account.rs` and `services/api_keys.rs` do the same, and the skill's layered model is preserved (services import no HTTP types; `AppError` is the shared error). Missing target → `NotFound` (404) per the spec scenarios (not 422); added `ConflictKind::CannotBlockSelf` → 409 `cannot_block_self`; last-admin reuses `CannotRemoveLastServerAdmin` → 409. ESI is kept out of the service: the handler pre-fetches the name/corp snapshot and passes it in (mirroring `complete_sso_callback`'s split), so the service stays HTTP-free.
- [x] 6.2 `grant_admin(pool, actor, target)`: 404 if account missing; idempotent no-op if already admin (no audit); else set flag + emit `ServerAdminGranted{AdminGrant}` in one tx.
- [x] 6.3 `revoke_admin(pool, actor, target)`: 404 if missing; no-op if not admin; last-admin guard inside the tx via `count_server_admins_tx` (409 + rollback if would hit zero); else clear flag + emit `ServerAdminRevoked` in one tx.
- [x] 6.4 `block_character(pool, actor, eve_character_id, reason, character_name, corporation_name)`: self-block guard (409 `cannot_block_self` if the eve_character_id belongs to the actor's own account, writes nothing); name/corp snapshot pre-fetched best-effort by the handler via the new `esi::public_info::fetch_character_block_snapshot` (tolerates failure → `None`); in one tx: insert block row (idempotent — no-op + no audit if already blocked); if the character resolves to an account, clear that account's tokens (`characters::clear_tokens_for_account`) and delete its sessions (new tx-scoped `db::sessions::delete_for_account_in_tx`); emit `EveCharacterBlocked{reason}` when newly inserted.
- [x] 6.5 `unblock_character(pool, actor, eve_character_id)`: 404 if not blocked; else delete row + emit `EveCharacterUnblocked` in one tx. No token/session restore.
- [x] 6.6 `list_accounts`, `list_blocks`, `search_characters(q, limit)`, and `list_audit_log` pass-throughs. The audit pass-through forwards the `target_type`/`target_id`/`target_name` filter axes in addition to `event_type`/`actor`/`before`, and clamps limit via `clamp_limit` (default 50, max 200).
- [x] 6.7 Unit tests (sqlx, as the existing service tests do): idempotent grant/block; last-admin guard reject+rollback (flag preserved) and self-revoke allowed when not last; revoke non-admin no-op; self-block rejected (writes nothing); block-with-account clears tokens + kills sessions; block-without-account is a bare insert; ESI-unavailable (snapshot `None`) still blocks; unblock 404 path; `clamp_limit` bounds. (The ESI→`None` mapping itself lives in `esi/public_info.rs` and is exercised at the service boundary via the `None` snapshot params.)

## 7. Handler layer + routing

- [x] 7.1 Added `backend/src/handlers/api/v1/admin.rs` with all eight handlers, each taking `AdminAccount` (the value is used where needed — grant/revoke/block/unblock pass `admin_id` as the audit actor; the read endpoints take `_admin` purely to gate). Bodies validated in the handler; responses wrapped in `ApiResponse`; service `AppError`s propagate. The block handler does the best-effort ESI snapshot fetch (`fetch_character_block_snapshot`) before calling the HTTP-free service.
- [x] 7.2 Added DTOs in `backend/src/dto/admin.rs` (request + response shapes; `From<DbModel>` impls; no DB model is serialised directly). Includes `AuditLogPageDto` with the `next_before` keyset cursor.
- [x] 7.3 Registered the `/api/v1/admin/*` routes in `lib.rs` (nested under `/api/v1/admin`) and populated `registered_admin_routes()` — the section-4 fail-closed coverage tests now exercise all eight real routes (401/403) instead of vacuously.
- [x] 7.4 Added `#[utoipa::path]` annotations to all eight handlers; registered the paths, the new schemas, and an `admin` tag in `openapi.rs`.
- [x] 7.5 Integration tests in `tests/admin.rs` (13 tests): accounts-with-characters list; search resolves to owning account; grant→revoke; grant 404; last-admin revoke 409; block→list→unblock; self-block 409; unblock 404; audit list + `event_type`/`target_id` filter + `next_before` cursor; case-insensitive `target_name` filter; non-admin 403; unauthenticated 401; bearer-key 401. Account-teardown semantics (token-clear + session-delete) are covered at the service layer (§6.7); the handler block path uses a fast-failing ESI client so the snapshot is null and tests stay hermetic.

## 8. HURL coverage

- [x] 8.1 Added `backend/tests/hurl/admin.hurl`: unauthenticated → 401 for every endpoint (+ a bearer-key → 401 cookie-only check); non-admin session → 403; admin-session flow: list accounts, search, grant then revoke (and the self-revoke last-admin 409), block/list/unblock (+ idempotent block, unblock-twice 404), audit list + `before` pagination + a target filter query. File header + README document the prerequisite cookie variables (admin_session / non_admin_session / grant_target_id / admin_account_id), matching the existing cookie-based files (session.hurl, account.hurl). The no-credential prefix (9 requests) was run through hurl against the live dev stack and passes; the admin-session flow is operator-run (needs an SSO-obtained cookie), like the other cookie files. NOTE: filtered by `target_id` rather than `target_name` in the smoke query because the audit row's target_name depends on ESI snapshot availability at block time (null when ESI is down); the case-insensitive `target_name` filter itself is covered by the `tests/admin.rs` integration test.
- [x] 8.2 Added `backend/tests/hurl/blocks.hurl`: a victim account's bearer key works (200) → an admin blocks its character (204) → the key is rejected with 401 `account_blocked` → state restored by unblock (204). Documented in the file header + README.

## 9. Frontend: admin shell + pages

- [x] 9.1 `frontend/src/routes/admin/+layout.server.ts`: load `/api/v1/me`; if `!is_server_admin`, throw a 404 (do not disclose existence). Forward cookies per the project's load-fetch pattern.
- [x] 9.2 `/admin/+page.svelte` overview (counts: admins, blocked characters) with its `+page.server.ts` load.
- [x] 9.3 `/admin/admins/+page.svelte` + `+page.server.ts`: list admin accounts (main character + portrait); "Add admin" → character-search (calls `GET /api/v1/admin/characters/search`) → confirm "Promote the account containing <name>?" → form action POSTing grant to the resolved `account_id`; per-row "Revoke" form action (disabled/last-admin-aware). Use the existing `ConfirmDialog` component.
- [x] 9.4 `/admin/blocks/+page.svelte` + `+page.server.ts`: list blocks (name, corp, reason, blocked_by, blocked_at); "Block character" form (EVE character ID + optional reason) → POST; per-row "Unblock" form action with confirm.
- [x] 9.5 `/admin/audit/+page.svelte` + `+page.server.ts`: filterable (event_type, actor, and target-first: `target_type`/`target_id`/`target_name` — name search being the primary human affordance), cursor-paginated (`before` → `next_before`) audit list; render each entry's target alongside its actor.
- [x] 9.6 `/blocked/+page.svelte`: informational landing for a blocked pilot (static; explains the block and points at contacting an admin).
- [x] 9.7 Add the "Admin" affordance to `GlobalNav` / `UserMenu`, shown only when `is_server_admin` (from the existing `/me` data the layout already loads).
- [x] 9.8 Add i18n message keys for all new UI copy (paraglide), per the `internationalisation` spec.
- [x] 9.9 Frontend unit/component tests (Vitest): layout 404-gates non-admins; admin nav affordance visibility; block form validation; character-search-to-grant resolves the account id; confirm dialogs wired.
- [x] 9.10 Playwright e2e: admin sees `/admin` and its pages; non-admin gets 404; grant→revoke flow; block→unblock flow; a blocked user lands on `/blocked`.

## 10. Drift + tidy

- [x] 10.1 `cargo sqlx prepare -- --all-targets` from `backend/`; commit the regenerated `.sqlx/` cache.
- [x] 10.2 Confirm `GET /api/v1/me` still returns `is_server_admin` unchanged (no DTO drift); grep for any prose claiming admin is bootstrap-only and update.

## 11. Verification

### Backend

- [x] 11.1 `cargo fmt --check` from `backend/`.
- [x] 11.2 `cargo clippy --all-targets --all-features -- -D warnings` from `backend/`.
- [x] 11.3 `cargo sqlx prepare --check -- --all-targets` from `backend/`.
- [x] 11.4 `cargo test --all-targets` from `backend/` — all unit + integration tests pass (226 lib + admin 13 + blocks 7 + audit_log 20 + others), including the admin-auth coverage test, last-admin/self-block guards, block teardown, SSO block rejection, and bearer block rejection. (Run repeatedly green this session.)
- [x] 11.5 Hurl run against the live dev stack (`http://localhost:5000`) with real sessions: `admin.hurl` no-credential prefix (9 reqs) ✓; admin-session flow (15 reqs: grant/idempotent/404/revoke/last-admin-409/block/list/idempotent/unblock/unblock-404/audit-list/pagination/filter) ✓; `blocks.hurl` (bearer→401 `account_blocked` + teardown + unblock restore) ✓ — confirmed the victim key survives the block and the character is `token_status: expired` post-unblock. `me.hurl` regression ✓. NOTE: the live non-admin **403** step needs a live non-admin *session*; the one provided had no live session row (its API key still worked), so the 403 path was verified via the integration test (`admin_endpoint_rejects_non_admin_403`) rather than live. `account.hurl` not re-run live (it soft-deletes the account — destructive); covered by integration tests.

### Frontend (all three are required by project policy — `pnpm test` alone is NOT sufficient)

- [x] 11.6 `pnpm --filter frontend test` — Vitest unit/component tests.
- [x] 11.7 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile).
- [x] 11.8 `pnpm --filter frontend run test:e2e` — Playwright e2e tests.

## 12. Wrap-up

- [x] 12.1 `openspec validate add-server-admin-and-block-list --strict` — must pass.
- [x] 12.2 Update memory: `project-frontend-status` (admin pages added) and note the admin/block model in a project memory if it isn't derivable from specs; cross-link `project-backend-auth-model` (now also covers `AdminAccount` + block enforcement).
