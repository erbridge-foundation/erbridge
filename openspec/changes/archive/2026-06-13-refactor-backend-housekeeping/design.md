# Design — refactor-backend-housekeeping

## Context

Each item is a small, reviewed defect or debt; the change is a batch so the mechanical churn (error plumbing touches nearly every service function) happens once. No behaviour changes are intended except the explicitly listed ones (reaping, shutdown, timeout, bind config, column drops).

## Goals / Non-Goals

**Goals:**
- Remove the two largest sources of repetitive noise (error mapping, `_in_tx` twins) without changing semantics.
- The database stops accumulating expired session rows.
- Deploys stop dropping in-flight requests; requests cannot hang forever.

**Non-Goals:**
- Layer-boundary enforcement, crypto module relocation, or any skill-rule restructuring (raised separately as skill amendments — they change project rules, not product behaviour).
- The N+1 / bulk-ESI work (owned by `optimize-entity-search`).
- Inbound/outbound rate limiting (owned by `add-esi-rate-limit-backoff`).

## Decisions

**`From<DbError> for AppError` + `From<sqlx::Error> for AppError`, both defaulting to `Internal`.** Bare `?` then covers the dominant "any DB failure is a 500" case. Typed translations (unique violation → 409, check violation → 400) stay as explicit `map_err` at exactly the call sites that want them — the explicitness is the point there. Anti-goal: a blanket `From` that guesses conflict types from constraint names globally; mapping stays local to the operation that knows its constraint.

**Executor generics over duplicated SQL.** Functions whose pool/tx twins are byte-identical become `async fn f(executor: impl PgExecutor<'_>, …)`. Where a function is only ever called inside a transaction after this change, only the tx form survives. No new twins may be added (review rule recorded in the db-layer module docs).

**Column drop is a plain migration.** `ALTER TABLE session DROP COLUMN csrf_state, DROP COLUMN add_character_mode;` — nothing reads them; the in-flight record (and, with `harden-auth-flow`, the state cookie) owns CSRF. The session-table spec's stated rationale ("persisted so a restart between SSO start and callback does not strand the user") is factually stale — the in-flight record is in-memory, and once the session row exists the OAuth dance is over. `eve_character.is_online` is **kept**: the spec reserves it for the deferred presence feature (SSE), unlike the session columns it has a planned consumer.

**Reaping rides the daily sweep.** `token_sweep::run_once` calls `db::sessions::delete_expired` at the end of each pass and logs the count. A dedicated ticker was considered — unnecessary; daily is ample (expired rows are already invisible to auth; this is hygiene, not correctness).

**Lifecycle.** `axum::serve(...).with_graceful_shutdown(signal_handler)` listening for SIGTERM + ctrl-c; `TimeoutLayer` (30 s) added to the router stack (excluding nothing — no streaming endpoints exist yet; revisit when SSE lands); `BIND_ADDR` env var parsed in `config.rs` with the current `0.0.0.0:3000` as default so no deployment change is required.

**Bearer-key auth in one query.** `find_by_hash` grows a join to `account` (status) and an `EXISTS` against `blocked_eve_character`, returning one row that the extractor maps to `Unauthorized` / `AccountSoftDeleted` / `AccountBlocked` / ok. Three sequential round-trips become one; behaviour identical, covered by the existing extractor tests.

**`set_main` returns the row.** Add `RETURNING` of the updated character to the second UPDATE; `services/account::set_main_character` drops its post-commit `list_for_account` + `find`.

## Risks / Trade-offs

- [Mechanical diff size] The error-plumbing sweep touches most services. → It is type-driven (compiler enforces completeness) and lands as its own commit within the change for reviewability.
- [Timeout layer vs long requests] 30 s could bite a slow ESI-backed search chain. → Search paths already degrade to `Unavailable` on failure; a timeout surfaces as 500/`Unavailable` rather than a hung connection. Tunable constant.
- [Graceful shutdown vs the sweep] `tokio::spawn`ed sweep is aborted at shutdown mid-run. → Acceptable today (the sweep is idempotent and daily); a drain hook is overkill until more jobs exist (see the deferred jobs-interface note).
- [Column drop irreversibility] Rollback of the migration cannot restore dropped values. → The values are write-only today; nothing can miss them.

## Migration Plan

One migration (column drops). Backend-only deploy. Land after `fix-transactional-integrity` (same files) and independently of the other proposals. Rollback: revert code; a reverse migration re-adds the columns as nullable/default if ever needed.
