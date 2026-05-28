# Tasks

## 1. DB layer: make `soft_delete` transactional

- [x] 1.1 Change `backend/src/db/accounts.rs::soft_delete` signature from `pool: &PgPool` to `tx: &mut Transaction<'_, Postgres>`. Update the body to execute against `&mut **tx`.
- [x] 1.2 Update the existing `soft_delete_sets_status` sqlx test to open a transaction, call `soft_delete(&mut tx, id)`, commit, then assert.
- [x] 1.3 Update any other in-crate callers of `soft_delete` that currently pass a pool (notably the sqlx test `resolve_or_create_skips_bootstrap_when_soft_deleted_admin_exists`, which calls `soft_delete(&pool, first)`) to use a transaction.

## 2. DB layer: `clear_tokens_for_account`

- [x] 2.1 Add `pub async fn clear_tokens_for_account(tx: &mut Transaction<'_, Postgres>, account_id: Uuid) -> Result<()>` to `backend/src/db/characters.rs`. The query SHALL `UPDATE eve_character SET encrypted_access_token = NULL, encrypted_refresh_token = NULL, access_token_expires_at = NULL, scopes = '{}', updated_at = now() WHERE account_id = $1`.
- [x] 2.2 Add sqlx test `clear_tokens_for_account_nulls_credential_columns_only`: insert an account with two characters (one with tokens, one without), call the function in a transaction, commit, then assert all four credential columns are NULL/empty on both rows while identity columns (`name`, `corporation_id`, `corporation_name`, `alliance_id`, `is_main`, `eve_character_id`, `account_id`) are unchanged.
- [x] 2.3 Add sqlx test `clear_tokens_for_account_only_touches_target_account`: insert two accounts each with a character, clear one, assert the other account's character is untouched.

## 3. Service layer: wire the atomic transaction

- [x] 3.1 In `backend/src/services/account.rs::delete_account`, after the `is_server_admin` guard, open a transaction with `pool.begin().await`, call `accounts::soft_delete(&mut tx, account_id)` then `characters::clear_tokens_for_account(&mut tx, account_id)`, then `tx.commit()`. Map errors via `AppError::Internal` consistently.
- [x] 3.2 Extend the existing `delete_account_allows_admin_when_another_admin_exists` and `delete_account_allows_non_admin` sqlx tests: before calling `delete_account`, insert at least one `eve_character` row for the account under test (with non-NULL tokens via direct SQL or a test helper); after `delete_account`, assert `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at` are NULL and `scopes` is `'{}'` on that character.
- [x] 3.3 Add a new sqlx test `delete_account_is_atomic_on_account_with_characters`: insert account + two characters with tokens, call `delete_account`, assert `account.status = 'soft_deleted'` AND both characters have credential columns cleared. (Covers the "Soft-delete clears EVE-credential columns on every linked character" scenario.)

## 4. Spec drift check

- [x] 4.1 Grep `backend/src/openapi.rs` and any DTO doc-comments for prose that asserts characters are unchanged on soft-delete; update if found so the OpenAPI description matches the new spec. (Likely no-op — current text is "soft-delete the caller's account.")

## 5. Verification (backend-only — this change does not touch frontend code)

- [x] 5.1 `cargo fmt --check` from `backend/`.
- [x] 5.2 `cargo clippy --all-targets --all-features -- -D warnings` from `backend/`.
- [x] 5.3 `cargo sqlx prepare -- --all-targets` from `backend/` after the new `sqlx::query!` invocation in §2.1; commit the regenerated `backend/.sqlx/` cache.
- [x] 5.4 `cargo test` from `backend/` — all unit + sqlx integration tests pass, including the new ones from §2 and §3.
- [x] 5.5 Hurl pass against the running dev stack: `hurl --test --variable base_url=http://localhost/api backend/tests/hurl/account.hurl` (and `me.hurl`, `characters.hurl` as smoke). The `account.hurl` flow already exercises `DELETE /api/v1/account` and the `account_soft_deleted` rejection; no new hurl file needed.

## 6. Wrap-up

- [x] 6.1 Update memory entry `project-soft-delete-tokens-open.md`: change description and body to say the policy is **implemented**, not just resolved-on-paper. Keep the cross-link to `project-backend-auth-model`.
- [x] 6.2 Run `openspec validate clear-tokens-on-soft-delete --strict` — must pass.

## 7. Cookie-clear fix (bundled — surfaced by §5.5 verification)

`refresh_session_cookie` middleware was overwriting `delete_account`'s cookie-clearing `Set-Cookie` with a refreshed JWT cookie. The bug predates this change, but is folded in because the spec promise ("response clears the session cookie") was already a lie and bundling lets us tell the truth in one pass.

- [x] 7.1 `backend/src/handlers/middleware.rs` — make `RefreshedJwtSlot` `pub`, add `pub fn suppress(&self)` that empties the slot.
- [x] 7.2 `backend/src/handlers/api/v1/account.rs::delete_account` — extract `Extension<RefreshedJwtSlot>`, call `.suppress()` after the service call succeeds, before writing the cleared cookie.
- [x] 7.3 Unit tests on `RefreshedJwtSlot::suppress`: clears a filled slot; is a no-op on an empty slot.
- [x] 7.4 Extend integration test in `backend/tests/openapi_strict.rs::delete_account_204` (or add a sibling) to assert the response has exactly one `Set-Cookie` header and that it clears the session (`Max-Age=0`).
- [x] 7.5 Update memory entry to drop the "known pre-existing bug" caveat — fix is shipped in this change.
- [x] 7.6 Re-run hurl: full `account.hurl` must pass cleanly (step 3 was the previously-failing assertion).
- [x] 7.7 Re-run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `openspec validate clear-tokens-on-soft-delete --strict` after the §7 patches.
