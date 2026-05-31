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

- [ ] 4.1 Add `AdminAccount(pub Uuid)` extractor (in `handlers/middleware.rs` or a sibling). Session-cookie only — it SHALL NOT consult API keys. Resolve the session → account_id, load the account, require `is_server_admin = TRUE`. Reject: 401 `unauthenticated` (no session), 403 `forbidden_admin_required` (authenticated non-admin). Add `AppError::ForbiddenAdminRequired` if not already representable.
- [ ] 4.2 Add `AppError::AccountBlocked` → HTTP 401 `account_blocked` in `error.rs`.
- [ ] 4.3 Unit tests for the extractor: no cookie → 401; non-admin cookie → 403; admin cookie → Ok; bearer key for an admin account → 401 (keys never confer admin).
- [ ] 4.4 Add an `admin_auth_coverage` integration test mirroring the existing v1 auth-coverage test: enumerate every registered `/api/v1/admin/*` route and assert each handler extracts `AdminAccount` (a route missing it fails the test). Add a `registered_admin_routes()` helper in `lib.rs` if needed.

## 5. Block enforcement: bearer branch + SSO callback

- [ ] 5.1 In the bearer branch of `AuthenticatedAccount` (`handlers/middleware.rs`), after the existing `soft_deleted` check, add `account_has_blocked_character` → reject with `AppError::AccountBlocked`. Do NOT add any block check to the session-cookie branch.
- [ ] 5.2 In `services/auth.rs::complete_sso_callback`, before any account/character write, call `db::blocks::is_eve_character_blocked`. If blocked: emit `AuditEvent::BlockedLoginRejected { eve_character_id }` (actor None, acting_as None — actor is NULL; the subject is in details) in its own short transaction (or a dedicated audit-only write), return a blocked error/outcome that the handler maps to a `/blocked` redirect. Ensure this runs for both the login and add-character paths (it precedes the `resolve_or_create` branch, so it covers both).
- [ ] 5.3 In `handlers/auth.rs::callback`, map the blocked outcome to a redirect to `/blocked` (no cookie set, no session).
- [ ] 5.4 Integration tests: blocked character login → no account/character/session written, `/blocked` redirect, `blocked_login_rejected` audit row; blocked character via add-character flow → not attached, audit row written; never-seen blocked id → no orphan account created; bearer request for a blocked account → 401 `account_blocked`; cookie request for a (now session-less) blocked account → 401 `unauthenticated`; non-blocked cookie request issues no `blocked_eve_character` query (assert via query log or by construction in a focused test).

## 6. Service layer: admin operations

- [ ] 6.1 Add `backend/src/services/admin.rs` with an `AdminError` enum (NotFound, TargetAccountNotFound→422, LastAdmin→409 `cannot_remove_last_server_admin`, CannotBlockSelf→409 `cannot_block_self`, Internal) mapped to `AppError` at the handler boundary (services must not import HTTP types — keep the mapping in the handler or return `AppError` directly per existing service style).
- [ ] 6.2 `grant_admin(pool, actor, target)`: 404 if account missing; idempotent no-op if already admin (no audit); else set flag + emit `ServerAdminGranted{AdminGrant}` in one tx.
- [ ] 6.3 `revoke_admin(pool, actor, target)`: 404 if missing; no-op if not admin; last-admin guard inside the tx via `count_server_admins_tx` (409 + rollback if would hit zero); else clear flag + emit `ServerAdminRevoked` in one tx.
- [ ] 6.4 `block_character(pool, actor, eve_character_id, reason)`: self-block guard (reject 409 `cannot_block_self` if the eve_character_id belongs to the actor's own account); fetch ESI public-info best-effort (name/corp; tolerate failure → null); in one tx: insert block row (idempotent — no-op + no audit if already blocked); if the character resolves to an account, clear that account's tokens (reuse `characters::clear_tokens_for_account`) and delete its sessions (`session_store.remove_all_for_account` or a tx-scoped equivalent); emit `EveCharacterBlocked{reason}` when newly inserted.
- [ ] 6.5 `unblock_character(pool, actor, eve_character_id)`: 404 if not blocked; else delete row + emit `EveCharacterUnblocked` in one tx. No token/session restore.
- [ ] 6.6 `list_accounts`, `list_blocks`, `search_characters(q, limit)`, and `list_audit_log` pass-throughs. The audit pass-through SHALL forward the `target_type`/`target_id`/`target_name` filter axes (added by `add-audit-log-target-columns`) in addition to `event_type`/`actor`/`before`, and clamp limit to a max (default when None).
- [ ] 6.7 Unit tests (mock or sqlx as the existing service tests do): idempotent grant/block; last-admin guard true/false incl. self-revoke allowed when not last; self-block rejected; block-with-account clears tokens + kills sessions; block-without-account is a bare insert; ESI-failure still blocks; unblock 404 path.

## 7. Handler layer + routing

- [ ] 7.1 Add `backend/src/handlers/api/v1/admin.rs` with handlers for: `GET /accounts`, `GET /characters/search`, `POST /accounts/:id/grant-admin`, `POST /accounts/:id/revoke-admin`, `GET /blocks`, `POST /blocks`, `DELETE /blocks/:eve_character_id`, `GET /audit`. Each takes `AdminAccount(admin_id)`. Validate request bodies in the handler; wrap responses in the `ApiResponse` envelope; map service errors to `AppError`.
- [ ] 7.2 Add DTOs in `backend/src/dto/admin.rs` (request + response shapes; `From<DbModel>` impls; never serialize DB models directly).
- [ ] 7.3 Register the `/api/v1/admin/*` routes in `lib.rs` (nested router) and add them to `registered_api_v1_routes()` / a new `registered_admin_routes()` for the coverage tests.
- [ ] 7.4 Add `#[utoipa::path]` annotations so the admin endpoints appear in the OpenAPI doc; update `openapi.rs` if it enumerates paths.
- [ ] 7.5 Integration tests for each handler (happy + key error paths): grant/revoke incl. last-admin 409; block incl. self-block 409 and account-teardown; unblock incl. 404; search; audit list + filter (incl. `target_name` case-insensitive + `target_type`/`target_id`) + pagination cursor; all `/admin/*` reject non-admin (403) and unauthenticated (401) and bearer (401).

## 8. HURL coverage

- [ ] 8.1 Add `backend/tests/hurl/admin.hurl`: unauthenticated → 401 and (where determinable) non-admin → 403 for each admin endpoint; with an admin session: list accounts, search, grant then revoke (and last-admin 409 path), list/post/delete blocks, audit list + `before` pagination + a `target_name` filter query. Document the prerequisite admin session/key variables in the file header, matching the existing hurl files' style.
- [ ] 8.2 Add a blocked-flow assertion to an existing or new hurl file: a bearer key whose account is blocked → 401 `account_blocked`.

## 9. Frontend: admin shell + pages

- [ ] 9.1 `frontend/src/routes/admin/+layout.server.ts`: load `/api/v1/me`; if `!is_server_admin`, throw a 404 (do not disclose existence). Forward cookies per the project's load-fetch pattern.
- [ ] 9.2 `/admin/+page.svelte` overview (counts: admins, blocked characters) with its `+page.server.ts` load.
- [ ] 9.3 `/admin/admins/+page.svelte` + `+page.server.ts`: list admin accounts (main character + portrait); "Add admin" → character-search (calls `GET /api/v1/admin/characters/search`) → confirm "Promote the account containing <name>?" → form action POSTing grant to the resolved `account_id`; per-row "Revoke" form action (disabled/last-admin-aware). Use the existing `ConfirmDialog` component.
- [ ] 9.4 `/admin/blocks/+page.svelte` + `+page.server.ts`: list blocks (name, corp, reason, blocked_by, blocked_at); "Block character" form (EVE character ID + optional reason) → POST; per-row "Unblock" form action with confirm.
- [ ] 9.5 `/admin/audit/+page.svelte` + `+page.server.ts`: filterable (event_type, actor, and target-first: `target_type`/`target_id`/`target_name` — name search being the primary human affordance), cursor-paginated (`before` → `next_before`) audit list; render each entry's target alongside its actor.
- [ ] 9.6 `/blocked/+page.svelte`: informational landing for a blocked pilot (static; explains the block and points at contacting an admin).
- [ ] 9.7 Add the "Admin" affordance to `GlobalNav` / `UserMenu`, shown only when `is_server_admin` (from the existing `/me` data the layout already loads).
- [ ] 9.8 Add i18n message keys for all new UI copy (paraglide), per the `internationalisation` spec.
- [ ] 9.9 Frontend unit/component tests (Vitest): layout 404-gates non-admins; admin nav affordance visibility; block form validation; character-search-to-grant resolves the account id; confirm dialogs wired.
- [ ] 9.10 Playwright e2e: admin sees `/admin` and its pages; non-admin gets 404; grant→revoke flow; block→unblock flow; a blocked user lands on `/blocked`.

## 10. Drift + tidy

- [ ] 10.1 `cargo sqlx prepare -- --all-targets` from `backend/`; commit the regenerated `.sqlx/` cache.
- [ ] 10.2 Confirm `GET /api/v1/me` still returns `is_server_admin` unchanged (no DTO drift); grep for any prose claiming admin is bootstrap-only and update.

## 11. Verification

### Backend

- [ ] 11.1 `cargo fmt --check` from `backend/`.
- [ ] 11.2 `cargo clippy --all-targets --all-features -- -D warnings` from `backend/`.
- [ ] 11.3 `cargo sqlx prepare --check -- --all-targets` from `backend/`.
- [ ] 11.4 `cargo test` from `backend/` — all unit + integration tests pass, including the admin-auth coverage test, last-admin/self-block guards, block teardown, SSO block rejection, and bearer block rejection.
- [ ] 11.5 Hurl pass against the running dev stack for `admin.hurl` (and a re-run of `account.hurl` / `me.hurl` smoke to confirm no regression).

### Frontend (all three are required by project policy — `pnpm test` alone is NOT sufficient)

- [ ] 11.6 `pnpm --filter frontend test` — Vitest unit/component tests.
- [ ] 11.7 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile).
- [ ] 11.8 `pnpm --filter frontend run test:e2e` — Playwright e2e tests.

## 12. Wrap-up

- [ ] 12.1 `openspec validate add-server-admin-and-block-list --strict` — must pass.
- [ ] 12.2 Update memory: `project-frontend-status` (admin pages added) and note the admin/block model in a project memory if it isn't derivable from specs; cross-link `project-backend-auth-model` (now also covers `AdminAccount` + block enforcement).
