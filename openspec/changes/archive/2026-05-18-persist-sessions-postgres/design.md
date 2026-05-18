## Context

`SessionStore` today is `Arc<RwLock<HashMap<String, Session>>>` (`backend/src/session.rs:16`). It is constructed empty on every process start (`backend/src/main.rs:33`) and never persisted. The session cookie is a signed JWT carrying the session ID; verification (in `backend/src/handlers/middleware.rs`) checks the JWT signature and then looks the session ID up in the in-memory map. On restart the JWT remains valid in the browser but the map is empty, so every cookie-authenticated request 401s until the user re-logs.

We already have Postgres in the stack (`PgPool` in `AppState`), all auth state we care about for sessions (`account_id`, `csrf_state`, `add_character_mode`) is small, plain data, and the foundation change established a singular-table-name convention with the `account`, `eve_character`, `api_key` tables. A `session` table is the smallest possible step that fixes restart-loss and gives us idle expiry at the same time.

The cookie itself does not need to change shape — it stays an HS256 JWT whose claim is the session ID — but its `exp` will be refreshed by the middleware so the browser-side lifetime tracks the server-side `expires_at`.

## Goals / Non-Goals

**Goals:**
- Sessions survive backend restarts.
- A session is auto-expired after 7 days of inactivity.
- Active users (visiting within the 7-day window) never have to re-login.
- Existing `SessionStore` callers compile unchanged (same method names and signatures, only `async` semantics preserved).
- Single, idempotent migration that can be applied on a running database (the `session` table doesn't yet exist).

**Non-Goals:**
- Hard maximum session lifetime independent of idle expiry (e.g., "force re-login every 30 days regardless of activity"). Out of scope; can be a follow-up.
- Server-wide "log out all sessions for account X" admin endpoint. The `list_session_ids_for_account` + `remove` pair already supports this primitive; surfacing it as an HTTP endpoint is a separate change.
- Migrating to Redis or `tower-sessions`. Postgres is sufficient at this scale and avoids a new dependency.
- Cleanup-as-a-background-task. Expired rows are rejected on read; physical removal piggybacks on a `DELETE WHERE expires_at < now()` issued opportunistically (see Decision 4).

## Decisions

### Decision 1: Postgres table, not Redis or `tower-sessions`

A new `session` table keyed on `session_id TEXT PRIMARY KEY` storing `account_id UUID NOT NULL`, `csrf_state TEXT`, `add_character_mode BOOL NOT NULL DEFAULT FALSE`, `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`, `last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now()`, `expires_at TIMESTAMPTZ NOT NULL`. Indexes: `expires_at` for cleanup, `account_id` for the existing `list_session_ids_for_account` helper.

**Alternatives considered:**
- *Redis*: faster reads and a TTL primitive, but adds a new infra dependency and a new failure mode. Session reads are already on the request hot path with a DB hit (account lookup), so the marginal cost of a Postgres lookup is small.
- *`tower-sessions` crate*: would replace our hand-rolled `SessionStore` with a community-maintained one. Tempting, but it pulls in its own cookie shape and middleware and would break the current "JWT-carrying-session-id" cookie design without a clear win at this scale.
- *Stateless JWT only*: drop the server-side store entirely and trust the JWT. Rejected because we already have revocation paths (logout, future "log out everywhere"), and stateless JWTs can't honour them mid-lifetime.

### Decision 2: Sliding idle expiry, not fixed lifetime

`expires_at = now() + 7 days` on session creation. On every authenticated request the middleware runs `UPDATE session SET last_seen_at = now(), expires_at = now() + interval '7 days' WHERE session_id = $1 AND expires_at > now()`. A row is treated as valid iff that `UPDATE` affected a row (i.e., the `WHERE` matched). This collapses "read + refresh" into one round-trip and atomically rejects already-expired rows.

**Alternatives considered:**
- *Refresh only on a threshold (e.g., > 1 day since `last_seen_at`)*: saves a write on burst traffic but complicates reasoning. Postgres can comfortably absorb one UPDATE per authenticated request at our scale.
- *Fixed lifetime with explicit refresh endpoint*: standard OAuth pattern, but heavier for a single-page web UI and requires frontend changes.

### Decision 3: Refresh the cookie JWT on every auth

When the middleware refreshes the row, it also re-issues the session cookie with a new `exp` claim 7 days out and sets `Set-Cookie` on the response. Without this the browser-side cookie would expire on its original `exp` while the DB row is still valid.

The JWT continues to carry only the session ID. No new claims; the signing key and algorithm (HS256) are unchanged. `verify_session_jwt` already exists; a sibling `mint_session_jwt(session_id, key) -> String` is the only new helper.

### Decision 4: Opportunistic cleanup, not a scheduled job

We have no background-task infrastructure. Expired rows are physically removed by issuing `DELETE FROM session WHERE expires_at < now()` from the middleware on a small probability (e.g., 1% of authenticated requests) or piggybacked onto logout. Stale rows do no harm in the meantime because the read path checks `expires_at > now()`.

**Alternative considered:** a tokio task on a 1-hour timer. Simpler operationally to defer until we need a job scheduler for something else (e.g., ESI refresh, character refresh).

### Decision 5: Keep the public `SessionStore` API stable

`SessionStore::{new, add, get, remove, list_session_ids_for_account}` remain. Internally the `HashMap` is replaced with a `PgPool` (or a thin wrapper holding one). The `new()` constructor changes signature to take a `PgPool`. Call sites in `main.rs` and the tests change; the auth handlers and middleware do not.

`Session` gains the new timestamp fields but its existing fields are unchanged.

### Decision 6: Test strategy

- Unit tests for the new DB layer with `#[sqlx::test]`, following the `rust-rest-api` skill's coverage rule.
- A `tests/api_keys.rs`-style integration test exercising the full middleware + DB path: create a session row, hit a protected endpoint, assert `last_seen_at` advanced and `expires_at` was pushed.
- A targeted test asserting that a row with `expires_at < now()` causes `401`.
- HURL: extend the existing auth flow to verify the cookie's `Set-Cookie` is re-issued on subsequent authenticated requests.

## Risks / Trade-offs

- **One extra DB write per authenticated request** → Acceptable at this scale; can be made conditional later if we ever hit write contention. The same transaction already touches `account` indirectly via `AuthenticatedAccount`, so the marginal latency is small.
- **JWT replay window grows from "until original exp" to "until DB row expires"** → Intentional. This is the whole point of the change. The session ID is still server-revocable via `remove`.
- **Clock skew between app and DB** → Not material because all expiry checks use `now()` in the database itself; the app never compares its own clock to `expires_at`.
- **Opportunistic cleanup leaks rows under sustained read-only traffic** → Bounded: each authenticated request has a small probability of running `DELETE`. Under any realistic load the table stays small (rows are ≤ 7 days old). Worst case is migrating to a scheduled task later.
- **Migration drops in-memory sessions on deploy** → Same as current behaviour (a deploy already logs everyone out today). No user-visible regression.
- **`#[sqlx::test]` requires a Postgres role with `CREATEDB` that owns the base database** → Already established by the local-Postgres workflow in `CONTRIBUTING.md`; no new requirement.

## Migration Plan

1. Land migration `…_create_session.sql` adding the `session` table.
2. Rewrite `backend/src/session.rs` against `PgPool`.
3. Add JWT refresh in `backend/src/handlers/middleware.rs` and `backend/src/handlers/cookie.rs`.
4. Update `main.rs` and tests to construct `SessionStore` from the pool.
5. Regenerate `.sqlx/` query cache (`cargo sqlx prepare`).
6. Deploy. Existing browser cookies become invalid on first request (server has no row for them) and the user is redirected to login — same as any deploy today. No data migration needed.

Rollback: revert the commits. The `session` table can stay in place (harmless) or be dropped with a follow-up migration; the previous code does not reference it.

## Open Questions

None blocking. The 7-day window is a product call and easy to tune (single literal in one SQL helper).
