# Refactor Backend Housekeeping

## Why

A backend review (2026-06-11) collected a set of low-risk debt items that individually don't justify a change but together meaningfully improve maintainability and operability: pervasive `.map_err(AppError::Internal)` boilerplate (hundreds of sites), duplicated `_in_tx` query pairs, vestigial session columns whose documented rationale is stale, an expired-session reaper that exists but is never called (expired rows accumulate forever), no graceful shutdown (deploys drop in-flight requests), no request timeout, a hardcoded bind address, a duplicated portrait-URL format, a hardcoded ESI base URL on one path, and assorted stale comments.

## What Changes

- Error plumbing: `impl From<sqlx::Error> for AppError` and `impl From<DbError> for AppError` (defaulting to `Internal`), letting bare `?` replace the `map_err` chains; services keep explicit mapping only where a typed conflict is produced.
- Collapse duplicated pool/tx function pairs (`insert_key`/`_in_tx`, `delete_for_account`/`_in_tx`, `delete_character`/`_in_tx`, `count_server_admins`/`_tx`, session deletes) via `impl PgExecutor<'_>` generics or by keeping only the tx variant.
- Drop the vestigial `session.csrf_state` and `session.add_character_mode` columns (nothing reads them; CSRF state lives in the in-flight store and, after `harden-auth-flow`, the state cookie).
- Actually reap expired sessions: the daily sweep calls the existing `delete_expired`.
- Service lifecycle: graceful shutdown on SIGTERM/SIGINT, a request-timeout layer, and a configurable bind address (`BIND_ADDR`, default `0.0.0.0:3000`).
- Small unifications: one `esi::portrait_url` (drop the duplicate in `services/account.rs`), injectable ESI base URL for the callback's public-info fetch (drop the hardcoded `…/latest/` URL), a params struct for `token_sweep::spawn`'s six loose arguments, `set_main` returns the updated row (`RETURNING`) instead of the caller re-listing all characters, single-query bearer-key auth (key + account-status + blocked check in one join), and stale-comment cleanup (`decrypt_token`'s `#[allow(dead_code)]`, plus a sweep of the audit catalogue's "Dormant" annotations against their actual emit sites — `make-audit-log-self-contained` cleared the live maps/ACL variants and the stale `ServerAdminRevoked` label was corrected during the audit-change review; the genuinely-dormant variants keep theirs).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `data-persistence`: the `session` table loses `csrf_state` / `add_character_mode`; expired-session reaping becomes a requirement rather than an anticipated path.
- `eve-sso-auth`: the callback requirement's session-row description drops the two carried columns.
- `project-infrastructure`: gains lifecycle requirements (graceful shutdown, request timeout, configurable bind address).

## Impact

- Backend only; no frontend changes. One migration (column drops).
- Touches most service files mechanically (error plumbing), `db/*` (pair collapse), `main.rs`/`lib.rs` (lifecycle), `session.rs`, `services/token_sweep.rs`.
- `cargo sqlx prepare` cache regenerates (column drops change inferred types).
- Depends on nothing; safest landed **after** `fix-transactional-integrity` to avoid churning the same service files concurrently.
