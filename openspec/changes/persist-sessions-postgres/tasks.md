## 1. Schema

- [x] 1.1 Add migration `backend/migrations/<next-ts>_create_session.sql`: `CREATE TABLE session ( session_id TEXT PRIMARY KEY, account_id UUID NOT NULL REFERENCES account(id) ON DELETE CASCADE, csrf_state TEXT, add_character_mode BOOL NOT NULL DEFAULT FALSE, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(), expires_at TIMESTAMPTZ NOT NULL );` plus indexes `CREATE INDEX session_expires_at_idx ON session(expires_at);` and `CREATE INDEX session_account_id_idx ON session(account_id);`. Singular table name per the foundation convention.
- [x] 1.2 Run `cargo test` once to confirm the migration applies cleanly against a fresh per-test DB spawned by `#[sqlx::test]`.

## 2. DB layer

- [x] 2.1 Create `backend/src/db/sessions.rs` (per the `rust-rest-api` skill's `db/` module layout). Implement: `insert(pool: &PgPool, session_id: &str, account_id: Uuid, csrf_state: Option<&str>, add_character_mode: bool) -> Result<()>` — inserts with `expires_at = now() + interval '7 days'`.
- [x] 2.2 In the same module, implement `refresh_and_get(pool: &PgPool, session_id: &str) -> Result<Option<Session>>` — a single `UPDATE session SET last_seen_at = now(), expires_at = now() + interval '7 days' WHERE session_id = $1 AND expires_at > now() RETURNING *`. Returns `None` if no row matched (missing or expired).
- [x] 2.3 Implement `delete(pool: &PgPool, session_id: &str) -> Result<()>`.
- [x] 2.4 Implement `list_ids_for_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<String>>` returning non-expired rows only.
- [x] 2.5 Implement `delete_expired(pool: &PgPool) -> Result<u64>` running `DELETE FROM session WHERE expires_at < now()`; returns affected row count.
- [x] 2.6 Wire `pub mod sessions;` in `backend/src/db/mod.rs`.
- [x] 2.7 Unit tests with `#[sqlx::test]` covering: insert + read; refresh advances `last_seen_at` and `expires_at`; refresh of an expired row returns `None` and does not touch the row; delete; `list_ids_for_account` excludes expired rows; `delete_expired` removes only expired rows.

## 3. Service / store rewrite

- [x] 3.1 Rewrite `backend/src/session.rs`: replace `Arc<RwLock<HashMap<String, Session>>>` with a struct holding `PgPool`. Keep the public methods (`new`, `add`, `get`, `remove`, `list_session_ids_for_account`) but change `new` to take `PgPool`. Have each method delegate to `db::sessions::*`. `get` SHALL call `refresh_and_get` so reading a session is also what extends it. Inflight OAuth records moved to a sibling `InflightStore` (in-memory by design — they have no `account_id` and are intentionally restart-volatile).
- [x] 3.2 Update the `Session` struct: add `created_at`, `last_seen_at`, `expires_at` (`chrono::DateTime<Utc>`); existing fields unchanged.
- [x] 3.3 Delete the old in-memory unit tests in `session.rs` (they're superseded by the DB layer tests in 2.7).

## 4. Cookie / JWT refresh

- [x] 4.1 In `backend/src/handlers/crypto.rs`, ensure `mint_session_jwt(session_id: &str, key: &[u8]) -> String` exists and produces a JWT with `exp = now() + 7 days`. If the current `verify_session_jwt` shares a mint helper, reuse it. (Implemented as `sign_session_jwt`, the existing name.)
- [x] 4.2 In `backend/src/handlers/cookie.rs`, confirm `set_session_cookie` produces the correct attributes; no change needed if it already mirrors `Max-Age`/`Expires` from the JWT (it currently doesn't set Max-Age, which is fine because the JWT's `exp` is the source of truth).

## 5. Middleware

- [x] 5.1 In `backend/src/handlers/middleware.rs`, change the session-cookie branch of `AuthenticatedAccount`: replace `state.session_store.get(&session_id)` with the path that goes through `refresh_and_get`. On a `None` return, respond `401`; on `Some`, attach the account ID to the request as today AND set a fresh `Set-Cookie` header on the response carrying a re-minted JWT.
- [x] 5.2 Setting the response cookie from a `FromRequestParts` extractor is not possible directly — restructure as a `tower` middleware (or per-handler concern) that wraps the response. Implemented via an axum `middleware::from_fn` layer (`refresh_session_cookie`) that installs a request-scoped `RefreshedJwtSlot`; the extractor fills the slot on cookie auth and the layer writes `Set-Cookie` on the way out.
- [x] 5.3 Ensure API-key requests (`erb_` bearer path) do NOT touch the session table and do NOT receive a refreshed cookie. Verify with a focused unit test on the middleware.

## 6. Auth handlers

- [x] 6.1 `backend/src/handlers/auth.rs` callback: replace the in-memory `session_store.add(...)` call with `db::sessions::insert(...)`; the session ID is still minted the same way, the JWT in the cookie still wraps it. (Add-character mode now inserts a fresh row with `add_character_mode = true`; we no longer try to update a duplicate keyed on the old session ID because the inflight is no longer in the same table.)
- [x] 6.2 `auth.rs` logout: replace the in-memory `remove` with `db::sessions::delete`. Cookie clearing is unchanged.
- [x] 6.3 No spec/route changes elsewhere.

## 7. Wiring

- [x] 7.1 In `backend/src/main.rs`, change `SessionStore::new()` to `SessionStore::new(pool.clone())`. Remove any remaining `Arc<RwLock<...>>` ceremony.
- [x] 7.2 `AppState`: `session_store` field type is unchanged at the call sites; only its internals changed. (Added a sibling `inflight_store: InflightStore` for OAuth in-flight records — see the design note on splitting in-memory inflight from the persisted session table.)

## 8. Tests

- [x] 8.1 Update existing integration tests in `backend/tests/api_keys.rs` and anything else that constructed the in-memory store to construct it from the test pool.
- [x] 8.2 Add `backend/tests/sessions.rs` covering the spec scenarios end-to-end against a real DB: session survives restart (simulate by dropping and rebuilding `AppState` between requests, reusing the pool); expired row is rejected (insert with `expires_at = now() - interval '1 second'`); cookie is reissued on success (assert `Set-Cookie` present with a parseable JWT); API-key request does not touch the session row (assert `last_seen_at` unchanged).
- [x] 8.3 Add HURL coverage in `backend/tests/hurl/` for the cookie-reissue behaviour on a normal protected request.

## 9. Build & verification

- [x] 9.1 Run `cargo sqlx prepare` to regenerate `.sqlx/` for the new queries; commit the updated cache. (Used `cargo sqlx prepare -- --all-targets` per the `rust-rest-api` skill; `--check` is clean.)
- [x] 9.2 `cargo build --release` produces zero warnings.
- [x] 9.3 Full `cargo test` suite passes locally and in CI. (Local: 69 lib + 4 api_keys + 7 openapi_strict + 4 sessions = 84 tests, all passing. `cargo clippy --all-targets -- -D warnings` is also clean.)
- [x] 9.4 Manual smoke: log in, restart the backend, refresh the browser — request succeeds, no re-login prompt. Inspect a response and confirm a refreshed `Set-Cookie` is present.
