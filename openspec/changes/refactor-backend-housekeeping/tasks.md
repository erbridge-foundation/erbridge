# Tasks — refactor-backend-housekeeping

## 1. Error plumbing

- [x] 1.1 Add `From<sqlx::Error> for AppError` and `From<DbError> for AppError` (→ `Internal`); sweep services/handlers replacing `map_err(AppError::Internal)` chains with `?`, keeping explicit maps only at typed-conflict sites (own commit for reviewability)
- [x] 1.2 `cargo clippy --all-targets -- -D warnings` clean after the sweep; spot-check that conflict mappings (api-key name, map slug, acl member) still produce their typed 409/400s via existing tests

## 2. db-layer pair collapse

- [x] 2.1 Convert byte-identical pool/tx twins to `impl PgExecutor<'_>` (api_keys insert/delete, characters delete, sessions delete_for_account, accounts count_server_admins); delete the redundant variants; note the no-new-twins rule in `db/mod.rs` docs
      NOTE: of the four named areas, only **api_keys** had a genuine byte-identical
      twin pair. `insert_key`/`insert_key_in_tx` collapsed to one
      `insert_key(executor: impl PgExecutor<'_>)`; the `delete_for_account`
      pair (pool→bool vs tx→Option<String>) unified to the richer
      `delete_for_account(executor) -> Option<String>` and the pool-only `bool`
      variant deleted. `characters::delete_character_owned_in_tx`,
      `characters::count_for_account_in_tx`, `sessions::delete_for_account_in_tx`
      are already single tx-only variants with no pool sibling — nothing to
      collapse. `accounts::count_server_admins` (lock-free pool read) and
      `count_server_admins_tx` (`FOR UPDATE` lock) are NOT byte-identical — the
      lock is semantically load-bearing — so both are kept; this is recorded in
      the no-new-twins note in `db/mod.rs`.
- [x] 2.2 Existing sqlx tests updated to the unified signatures

## 3. Session columns + reaping

- [x] 3.1 Migration: `ALTER TABLE session DROP COLUMN csrf_state, DROP COLUMN add_character_mode`; strip the fields from `SessionRow`/`Session` and `SessionStore::add`'s signature
- [x] 3.2 Call `db::sessions::delete_expired` at the end of `token_sweep::run_once`, logging the count; sqlx test that the sweep pass removes expired rows
- [x] 3.3 `cargo sqlx prepare -- --all-targets` and commit the cache diff

## 4. Lifecycle

- [x] 4.1 `with_graceful_shutdown` on SIGTERM/ctrl-c in `main.rs`; `TimeoutLayer` (30 s) in the router stack; `BIND_ADDR` in `config.rs` (default `0.0.0.0:3000`)
- [x] 4.2 Tests: config default + parse; timeout layer behaviour (integration test with a deliberately slow test route or unit-level layer test)

## 5. Small unifications

- [x] 5.1 Use `esi::portrait_url` in `services/account.rs` (delete the inline format)
- [x] 5.2 Inject the ESI base URL into `handlers/auth.rs::fetch_character_public_info` (drop the hardcoded `https://esi.evetech.net/latest/`), aligning with the injectable-base pattern used by search
- [x] 5.3 Bundle `token_sweep::spawn`'s six parameters into a `SweepContext` struct
- [x] 5.4 `set_main` second UPDATE gains `RETURNING`; `set_main_character` drops the post-commit re-list
- [x] 5.5 Single-query bearer auth: extend `db/api_keys::find_by_hash` with account-status + blocked-character join; extractor maps the combined row; existing extractor/middleware tests cover behaviour
      NOTE: `find_by_hash` now returns a purpose-built `BearerKeyRow` (scope,
      account_id, `account_status?` via LEFT JOIN account, `account_blocked!` via
      EXISTS subquery) instead of the old `ApiKeyRow`; the latter struct (only
      ever this fn's return) was deleted. The bearer branch in `middleware.rs`
      collapses three queries (`find_by_hash` → `get_account` →
      `account_has_blocked_character`) into the one lookup and matches on
      `account_status.as_deref()`. With the bearer path being its sole production
      caller, `db::blocks::account_has_blocked_character` (and its three dedicated
      unit tests + the `bind_character` helper) became dead code and were removed;
      its EXISTS logic is now inlined in `find_by_hash` and covered by the new
      `find_by_hash_reports_blocked_account` / `_reports_soft_deleted_status` db
      tests plus the existing `tests/blocks.rs` bearer integration tests.
- [x] 5.6 Comment hygiene: remove `decrypt_token`'s stale `#[allow(dead_code)]` + comment; fix the `token_encryption_key` "padded with zeros" doc if not already fixed by harden-token-crypto. Audit catalogue "Dormant" annotations: verify each against its actual emit site rather than trusting the label — `make-audit-log-self-contained` cleared them from the live maps/ACL variants, and the stale `ServerAdminRevoked` label (it *is* emitted by `services::admin::revoke_admin`) was corrected during the audit-change review. The labels that remain (`AccountPurged`, the four `Admin*`-override map/ACL variants) are genuinely dormant — confirmed no emit site — leave them.
      NOTE: `decrypt_token` — removed the `#[allow(dead_code)]` and the
      "forthcoming change" doc; it is now actively called by `token_sweep` and
      `entity_search`, so the doc was rewritten to name those callers and clippy
      stays clean without the allow. `token_encryption_key` "padded with zeros"
      doc — already corrected by harden-token-crypto (no such phrase remains in
      `crypto.rs`); nothing to do. Dormant annotations — verified by grepping
      every variant's constructor: the five labelled variants (`AccountPurged`,
      `AdminMapOwnershipChanged`, `AdminMapHardDeleted`,
      `AdminAclOwnershipChanged`, `AdminAclHardDeleted`) appear nowhere outside
      `audit/mod.rs` → genuinely dormant, labels accurate. `ServerAdminRevoked`
      confirmed emitted at `services/admin.rs:123` and correctly carries no
      dormant label. No catalogue changes needed.

## 6. Verification

- [x] 6.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`
      NOTE: fmt + clippy (`-D warnings`) clean; full suite green — 520 tests
      (413 lib + admin 17 + api_keys 6 + audit_log 20 + auth 5 + blocks 7 +
      health 1 + layering 4 + maps_acls 10 + me 2 + openapi_strict 18 +
      preferences 9 + rate_limit 4 + sessions 4), 0 failed. One pre-existing
      failure surfaced and was fixed in passing: `maps_acls::deny_member_refuses_access`
      POSTed both `read` and `deny` to the *same* corp member, which migration
      10's `acl_member_unique_entity` index (acl, member_type, eve_entity_id —
      permission NOT in the key) rejects with 409, so the deny never stored and
      the member read through. Rewrote it to grant `read` to the corp and `deny`
      the member's *character* (distinct identity), mirroring the passing
      `permissions::deny_overrides_all_grants` unit test; the resolver's deny-wins
      logic was correct and untouched.
- [x] 6.2 Full HURL suite against the dev compose stack (no contract changes expected — this proves it)
      NOTE: ran live against the dev stack (traefik :5000). PASS: health, me,
      keys, preferences, maps, acls, entities, admin (full 35-request flow:
      no-cred 401s, non-admin 403, list/search, grant→revoke incl. last-admin
      409, block/list/unblock, audit list + pagination + target_id filter), and
      **blocks** (4 req) — the strongest live proof of §5.5: admin blocks the
      victim's character → the victim's API key is rejected 401 `account_blocked`
      via the new EXISTS join in `find_by_hash`, then unblock restores 200. Two
      pre-existing HURL-fixture quirks, NOT contract regressions and NOT caused by
      this change, left as-is (they belong to other changes' fixtures):
      (a) `session.hurl` step 2 expects a cookieless `/me`→401 but hurl's
      per-file cookie jar replays the step-1 reissued cookie → 200; a genuinely
      cookieless `curl /me` returns 401 as required. (b) `auth.hurl` step 1
      asserts `/auth/login`→302 but axum `Redirect::to` emits 303 (harden-auth-flow
      fixture drift). Operational gotchas worth recording: the inbound per-IP
      governor throttles rapid back-to-back files (run individually with spacing);
      map slug UNIQUE is status-agnostic so a soft-deleted `smoke-chain` keeps the
      slug reserved (purge soft-deleted rows between maps.hurl runs); and the
      session-cookie JWT `sub` is the **session_id**, not the account_id — resolve
      account_id via the `session` table before using admin.hurl's
      `admin_account_id`/`grant_target_id`.
- [x] 6.3 Live smoke: `docker compose` deploy-restart under a long-running curl to confirm graceful drain; confirm `BIND_ADDR` override works
      NOTE: both validated against the dev stack (image already built from this
      working tree). BIND_ADDR override — set `BIND_ADDR=0.0.0.0:3999` via a
      compose env override: traefik (→3000) immediately returned 502 and
      `/proc/net/tcp` inside the container showed the listener on **3999** (not
      3000); restoring `0.0.0.0:3000` returned health to 200. Conclusive proof the
      env var is honored. Graceful shutdown — sending SIGTERM directly to the
      backend (PID 1) produced a clean **exit code 0** in ~0.26s (not SIGKILL/137,
      not OOM), reproduced twice; a connection held in-flight (slow-trickle HTTP
      client) at the moment of SIGTERM still received a complete HTTP response
      rather than a reset before the process exited 0 — i.e. `with_graceful_shutdown`
      drained instead of dropping it. (Aside: the `tracing::info!("listening on …")`
      / "received SIGTERM" boot lines don't surface because the dev compose
      `RUST_LOG=erbridge=debug,…` filter targets `erbridge` while the crate is
      `backend`, so info-level logs from the binary are filtered — cosmetic, not a
      bug in this change.) Stack restored to the plain compose definition (no
      override), all services up, health 200.
