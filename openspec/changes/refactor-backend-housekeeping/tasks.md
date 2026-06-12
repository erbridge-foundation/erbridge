# Tasks — refactor-backend-housekeeping

## 1. Error plumbing

- [ ] 1.1 Add `From<sqlx::Error> for AppError` and `From<DbError> for AppError` (→ `Internal`); sweep services/handlers replacing `map_err(AppError::Internal)` chains with `?`, keeping explicit maps only at typed-conflict sites (own commit for reviewability)
- [ ] 1.2 `cargo clippy --all-targets -- -D warnings` clean after the sweep; spot-check that conflict mappings (api-key name, map slug, acl member) still produce their typed 409/400s via existing tests

## 2. db-layer pair collapse

- [ ] 2.1 Convert byte-identical pool/tx twins to `impl PgExecutor<'_>` (api_keys insert/delete, characters delete, sessions delete_for_account, accounts count_server_admins); delete the redundant variants; note the no-new-twins rule in `db/mod.rs` docs
- [ ] 2.2 Existing sqlx tests updated to the unified signatures

## 3. Session columns + reaping

- [ ] 3.1 Migration: `ALTER TABLE session DROP COLUMN csrf_state, DROP COLUMN add_character_mode`; strip the fields from `SessionRow`/`Session` and `SessionStore::add`'s signature
- [ ] 3.2 Call `db::sessions::delete_expired` at the end of `token_sweep::run_once`, logging the count; sqlx test that the sweep pass removes expired rows
- [ ] 3.3 `cargo sqlx prepare -- --all-targets` and commit the cache diff

## 4. Lifecycle

- [ ] 4.1 `with_graceful_shutdown` on SIGTERM/ctrl-c in `main.rs`; `TimeoutLayer` (30 s) in the router stack; `BIND_ADDR` in `config.rs` (default `0.0.0.0:3000`)
- [ ] 4.2 Tests: config default + parse; timeout layer behaviour (integration test with a deliberately slow test route or unit-level layer test)

## 5. Small unifications

- [ ] 5.1 Use `esi::portrait_url` in `services/account.rs` (delete the inline format)
- [ ] 5.2 Inject the ESI base URL into `handlers/auth.rs::fetch_character_public_info` (drop the hardcoded `https://esi.evetech.net/latest/`), aligning with the injectable-base pattern used by search
- [ ] 5.3 Bundle `token_sweep::spawn`'s six parameters into a `SweepContext` struct
- [ ] 5.4 `set_main` second UPDATE gains `RETURNING`; `set_main_character` drops the post-commit re-list
- [ ] 5.5 Single-query bearer auth: extend `db/api_keys::find_by_hash` with account-status + blocked-character join; extractor maps the combined row; existing extractor/middleware tests cover behaviour
- [ ] 5.6 Comment hygiene: remove `decrypt_token`'s stale `#[allow(dead_code)]` + comment; fix the `token_encryption_key` "padded with zeros" doc if not already fixed by harden-token-crypto. Audit catalogue "Dormant" annotations: verify each against its actual emit site rather than trusting the label — `make-audit-log-self-contained` cleared them from the live maps/ACL variants, and the stale `ServerAdminRevoked` label (it *is* emitted by `services::admin::revoke_admin`) was corrected during the audit-change review. The labels that remain (`AccountPurged`, the four `Admin*`-override map/ACL variants) are genuinely dormant — confirmed no emit site — leave them.

## 6. Verification

- [ ] 6.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`
- [ ] 6.2 Full HURL suite against the dev compose stack (no contract changes expected — this proves it)
- [ ] 6.3 Live smoke: `docker compose` deploy-restart under a long-running curl to confirm graceful drain; confirm `BIND_ADDR` override works
