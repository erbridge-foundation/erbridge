## Why

The current session store is an in-memory `HashMap` (`backend/src/session.rs`), so every backend restart silently logs every browser user out — the signed session cookie survives in the browser, but the server-side lookup that maps session ID → `account_id` is gone, and the next request returns `401`. For a single-instance app this is purely an operational papercut; it will become a correctness problem the moment we run more than one backend replica. We also have no notion of session expiry: a session created on day 1 is valid indefinitely as long as the server hasn't restarted, which is the wrong default for a tool that holds EVE ESI credentials by proxy.

## What Changes

- **BREAKING**: Replace the in-memory `SessionStore` with a Postgres-backed store. Existing in-memory sessions are dropped on deploy (this is the current behaviour anyway, just made explicit).
- Add a `session` table (singular, per the foundation schema convention) keyed by session ID with `account_id`, `csrf_state`, `add_character_mode`, `created_at`, `last_seen_at`, `expires_at`.
- Introduce a **sliding 7-day idle expiry**: each authenticated request updates `last_seen_at` and pushes `expires_at` to `now() + 7 days`. A session not touched for 7 days is rejected as `401` and treated as deleted.
- On every successful authenticated request the session cookie's JWT is refreshed (re-signed with a new `exp`) so the browser also carries a fresh 7-day token. This avoids the cookie expiring before the server-side row does.
- Sessions whose `expires_at` is in the past SHALL be rejected on read. A periodic cleanup (DB-level) removes them; eviction does not need to be synchronous with auth.
- `GET /auth/logout` deletes the row (already does in spirit — now hits Postgres).
- `SessionStore` retains the same surface (`add`, `get`, `remove`, `list_session_ids_for_account`) so callers don't change; only the implementation swaps `Arc<RwLock<HashMap<…>>>` for a `PgPool` wrapper.

Explicitly out of scope: a server-wide "log everyone out" admin endpoint, Redis as a backing store, hard maximum session lifetime independent of activity.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `eve-sso-auth`: session storage moves from in-memory to Postgres; sessions gain a sliding 7-day idle expiry; the session cookie JWT is refreshed on each authenticated request.

## Impact

- **Code**: `backend/src/session.rs` (rewrite of `SessionStore` against `PgPool`), `backend/src/handlers/middleware.rs` (touch `last_seen_at` / refresh cookie on auth), `backend/src/handlers/cookie.rs` (set fresh JWT on response), `backend/src/handlers/auth.rs` (no logic change, but session creation hits DB), `backend/src/main.rs` (drop `SessionStore::new()`, construct from pool).
- **Schema**: new migration `…_create_session.sql` adding the `session` table with an index on `(expires_at)` for cleanup and `(account_id)` for `list_session_ids_for_account`.
- **Tests**: unit tests for the new DB layer (`#[sqlx::test]`), update existing middleware/auth tests to use a DB-backed store, HURL coverage for "session survives restart" (proxied by directly inserting a row), "expired session is rejected", "session is refreshed on activity".
- **Dependencies**: no new crates — `sqlx`, `chrono`, `uuid` already in the workspace.
- **Operational**: no migration of existing sessions (they're already volatile). One-time `cargo sqlx prepare` regeneration for the offline query cache.
