# Tasks — fix-transactional-integrity

## 1. Soft-delete session atomicity

- [x] 1.1 Move session deletion into `services/account::delete_account`'s transaction (use `db/sessions::delete_for_account_in_tx`); evaluate the last-admin guard inside the same transaction (reuse `count_server_admins_tx` after the status flip, rollback + 409 on violation)
- [x] 1.2 Slim `handlers/api/v1/account.rs` to response concerns only (cookie clear + `RefreshedJwtSlot::suppress`); remove the handler-side `remove_all_for_account` call
- [x] 1.3 Tests: sessions gone after soft-delete within one tx (assert via injected failure that partial states roll back); concurrent last-two-admins delete → exactly one 409

## 2. Character-delete guard atomicity

- [x] 2.1 Restructure `services/account::delete_character`: open tx → delete with `RETURNING is_main` and ownership in the WHERE → re-check remaining count inside tx → rollback + 409 (`cannot_remove_last_character` / `cannot_remove_main`) on violation → audit → commit
- [x] 2.2 Extend/replace `db/characters` helpers as needed (tx-scoped count; drop the now-unused pool-side pre-checks)
- [x] 2.3 Tests: existing 409/404 behaviour preserved; concurrent-delete race test leaves ≥1 character with a main

## 3. ACL transactional restructure

- [x] 3.1 Rework `services/acl.rs` so each mutation runs ownership check → write → audit in one transaction (`add_member`, `update_member_permission`, `remove_member`, `rename_acl`, `delete_acl`); add tx variants in `db/acl.rs` / `db/acl_member.rs` where missing and remove orphaned pool variants
- [x] 3.2 Replace message-substring CHECK detection in `map_member_db_err` with SQLSTATE `23514` matching via `sqlx::Error::Database` (extend `DbError` with a `CheckViolation { constraint }` variant)
- [x] 3.3 Tests: audit-atomicity (failed audit rolls back mutation); CHECK violation still maps to 400

## 4. acl_member uniqueness

- [x] 4.1 Migration: dedupe existing `acl_member` duplicates (keep oldest), then create `acl_member_unique_character (acl_id, character_id) WHERE member_type = 'character'` and `acl_member_unique_entity (acl_id, member_type, eve_entity_id) WHERE member_type <> 'character'`
- [x] 4.2 Add `ConflictKind::DuplicateAclMember` (409 `duplicate_acl_member`); map `DbError::UniqueViolation` on member insert to it; update OpenAPI annotations
- [x] 4.3 Frontend: surface the 409 message in the member picker (new i18n key in en/de/fr); no structural change
- [x] 4.4 Tests: sqlx duplicate-insert tests per member type; same-entity-different-type allowed; HURL 409 assertion; migration dedupe test

## 5. Verification

- [x] 5.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`; `cargo sqlx prepare -- --all-targets` and commit the cache diff — all green (clippy clean; 409 lib + all integration tests pass; .sqlx cache regenerated)
- [x] 5.2 `pnpm --filter frontend test` — Vitest unit/component tests — **291 pass** (run as `pnpm test` from `frontend/`; this repo has no workspace root so `--filter` is unavailable)
- [x] 5.3 `pnpm --filter frontend run check` — svelte-check — **0 errors / 0 warnings** (run as `pnpm run check` from `frontend/`)
- [x] 5.4 `pnpm --filter frontend run test:e2e` — Playwright e2e tests — **25 pass** (run as `pnpm run test:e2e` from `frontend/`)
- [x] 5.5 Live HURL run against the dev compose stack covering the duplicate-member 409 — `acls.hurl` **13/13 pass** against the rebuilt backend (migration verified applied live; step 6b asserts `error.code == "duplicate_acl_member"`). `me.hurl`/`entities.hurl` also pass; `characters.hurl` (needs externally-supplied `non_main_character_id`) and `maps.hurl` (slug-already-exists from leftover dev data) fail for reasons unrelated to this change.
