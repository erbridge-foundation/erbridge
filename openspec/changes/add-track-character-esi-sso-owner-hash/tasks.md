## 1. Schema

- [ ] 1.1 Add migration `ALTER TABLE eve_character ADD COLUMN owner_hash TEXT;` (nullable, no default) as the next sequential file in `backend/migrations/`.
- [ ] 1.2 Run the migration against the local dev/test DB so the sqlx compile-time checks see the new column.

## 2. DB layer (`backend/src/db/`)

- [ ] 2.1 In `db/characters.rs`, add `owner_hash` to the `Character` struct and include it in the `SELECT` / row mapping of every function that returns a `Character`.
- [ ] 2.2 Thread `owner_hash` through `upsert_tokens` and `create_orphan` (and any other insert/claim path) so it is written on first link, orphan-claim, and re-auth.
- [ ] 2.3 Extend the existing eve-character lookup used by the callback (`find_account_id_for_eve_character` or equivalent) to also return the stored `owner_hash` and current `account_id`, so the service can compare in one round-trip rather than adding a second query.
- [ ] 2.4 Confirm `clear_tokens_for_account(tx, account_id)` already NULLs `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, and empties `scopes`; reuse it for the previous-owner token wipe (extend only if a column is missing).
- [ ] 2.5 Add a `db/characters.rs` function to detach a character: set `account_id = NULL`, `is_main = FALSE`, and store the new `owner_hash`, by internal UUID, within the transaction. Returns nothing or the row id.
- [ ] 2.6 Confirm session clearing for an account exists (`db/sessions.rs::delete_for_account` via `SessionStore`); ensure a transaction-scoped variant is available for use inside the callback transaction (add a `*_in_tx` sibling if the existing one takes a pool, mirroring the `api_keys` `delete_for_account_in_tx` pattern).
- [ ] 2.7 Unit tests (`#[sqlx::test]`, one function per test): owner_hash round-trips through upsert/orphan/select; the extended lookup returns the stored owner_hash + account_id; detach nulls account_id + is_main and updates owner_hash; clear-tokens and session-delete affect only the target account.

## 3. Audit event (`backend/src/audit/mod.rs`)

- [ ] 3.1 Add a `CharacterTransferDetected { eve_character_id, old_account_id, new_account_id }` variant following the existing dormant-variant house style (kind string, serialisation, target mapping).
- [ ] 3.2 Unit test: the variant serialises with the expected kind string and payload fields.

## 4. Service layer (`backend/src/services/auth.rs`)

- [ ] 4.1 Add `owner_hash` to the callback service input struct and thread it from the handler.
- [ ] 4.2 In the callback persistence transaction, after the lookup: if the row exists with `account_id` set, the stored `owner_hash` is non-null, and it differs from the presented claim, run transfer enforcement BEFORE the upsert — (a) `clear_tokens_for_account` for the previous owner, (b) delete the previous owner's sessions (in-tx), (c) emit the `CharacterTransferDetected` audit event with old/new account ids, (d) detach the row (task 2.5). Then fall through to the existing orphan-claim path.
- [ ] 4.3 Ensure first-seen rows, orphan rows, and null/equal stored hashes skip enforcement and simply record the presented owner_hash. Ensure only the previous owner's sessions are cleared — never the resolved (authenticating) account's.
- [ ] 4.4 Unit tests (mock the db layer): changed hash on an account_id-set row triggers wipe + session-delete + audit + detach then claim; equal hash does not; null stored hash does not; first-seen/orphan paths record the hash; the resolved account's session is preserved.

## 5. Handler layer (`backend/src/handlers/auth.rs`)

- [ ] 5.1 Add `owner: String` to `EsiJwtClaims` (the `owner` claim is always present on ESI access tokens; treat absence as a `BadGateway` parse error, consistent with the existing `sub` handling).
- [ ] 5.2 Pass the parsed `owner` into the service input struct (task 4.1). No other handler logic changes.
- [ ] 5.3 Unit test: `parse_esi_jwt_claims` extracts the `owner` claim; a JWT missing `owner` is rejected.

## 6. Integration & HURL coverage (`backend/tests/`)

- [ ] 6.1 Integration test (`#[sqlx::test]`, full callback handler→service→db): a re-auth with an unchanged owner hash behaves as today and records the hash.
- [ ] 6.2 Integration test: a callback presenting a changed owner hash for a character owned by account A, authenticating as account B, results in — A's characters token-wiped, A's sessions deleted, an audit row written, and the character re-linked to B with fresh tokens, new owner_hash, and promoted to main if B had no main.
- [ ] 6.3 Update/extend `tests/hurl/` callback coverage to assert the post-transfer HTTP outcome (successful redirect + session cookie for the new owner). Note: the owner-hash change is driven by JWT contents, so the transfer-specific assertions live primarily in the integration tests; the HURL file covers the endpoint contract.

## 7. Tooling & verification

- [ ] 7.1 Regenerate the sqlx offline cache from `backend/`: `cargo sqlx prepare -- --all-targets`, and commit the `.sqlx/` diff.
- [ ] 7.2 `cargo fmt` and `cargo clippy --all-targets` clean.
- [ ] 7.3 `cargo test` (all backend unit + integration tests) passes.
- [ ] 7.4 `cargo sqlx prepare --check -- --all-targets` passes (no cache drift).
- [ ] 7.5 Run the HURL suite against a running backend; the callback file passes.

> This change is backend-only; no frontend code is touched, so the frontend verification trio (`pnpm --filter frontend test` / `run check` / `run test:e2e`) does not apply. If implementation discovers a frontend touch, add those three commands here per `CLAUDE.md` before completing.
