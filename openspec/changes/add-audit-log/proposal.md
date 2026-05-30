## Why

The backend has no record of *who did what, when*. Today an admin investigating "did someone change Bob's main last week?" or "when did the first account get bootstrapped to server admin?" has nothing but git history and process logs to fall back on — neither of which carries the actual database actor or the affected EVE character. Once `is_server_admin` and the block-list land (and later, maps + ACLs), the absence of an audit trail goes from awkward to dangerous: admin actions affect other users and need to be attributable and reversible-with-confidence.

A prior iteration of this codebase (preserved under `zz-ref/backend/older-iteration`) shipped a well-shaped `audit_log` design — table, Rust-side enum vocabulary, transactional write helper, keyset-paginated read. Porting it now, *before* the admin/block change, means every sensitive endpoint (existing and future) emits audit events from day one. Retrofitting audit after the fact reliably produces gaps in the actions that matter most.

## What Changes

- **NEW** `audit-log` capability: an append-only `audit_log` table, a Rust `AuditEvent` enum encoding the catalogue of recordable actions, a `record_in_tx(...)` helper that participates in the caller's transaction, and a `list_audit_log(...)` read helper used by future admin tooling. Audit rows are INSERT-only; no UPDATE / DELETE path exists in application code, ever.

  The schema captures, per row: `occurred_at`, optional `actor_account_id` (FK with `ON DELETE SET NULL`), snapshotted `actor_character_id` (EVE BIGINT) and `actor_character_name` (TEXT) — the **main character** of the actor at the moment of write, so the human-readable thread survives even if the account row is later hard-deleted — `event_type`, and a per-event `details` JSONB payload.

  No IP or User-Agent capture in this change. The threat model (small EVE wormhole-mapper community) does not justify the cost of correct `X-Forwarded-For` parsing behind Traefik, and the columns can be added with no retrofit pain when a real need arises.

- **MODIFIED** `eve-sso-auth`: the OAuth2 callback handler SHALL emit `account_registered`, `orphan_character_claimed`, `account_reactivated`, and `server_admin_granted` (with source `first_account_bootstrap`) into the same transaction that performs each of those actions. Emissions occur *after* the `is_main = TRUE` promotion so the actor-character snapshot is non-null for every committed row.

- **MODIFIED** `account-management`: `DELETE /api/v1/account` SHALL emit `account_deletion_requested`; `POST /api/v1/characters/:id/set-main` SHALL emit `character_set_main`; `DELETE /api/v1/characters/:id` SHALL emit `character_removed`. All three within the existing transaction.

- **MODIFIED** `api-authentication`: `POST /api/v1/keys` SHALL emit `api_key_created` and `DELETE /api/v1/keys/:id` SHALL emit `api_key_revoked`, both within the request's existing transaction.

- The full `AuditEvent` enum vocabulary is ported from the older iteration with two renames (`ghost_character_claimed` → `orphan_character_claimed`; current code uses "orphan"). Variants for features that don't yet exist in this codebase (`account_purged`, `map_*`, `acl_*`, admin-override events, `eve_character_blocked` / `unblocked`, `server_admin_revoked`, the `server_admin_granted{admin_grant}` source) are present in the enum but not emitted by any v1 code path; they activate when the features that need them land. Per-variant unit tests cover the serialization shape from day one; integration tests cover only the variants v1 emits.

## Capabilities

### New Capabilities

- `audit-log`: append-only audit trail capability — the `audit_log` table, the actor-resolution rules (account, acting-character, system), the `AuditEvent` catalogue, the write contract (`record_in_tx` semantics, INSERT-only invariant, transactional participation, fail-soft on main lookup with `tracing::error`), and the read contract (filterable by `event_type` / `actor_account_id` / `before` cursor).

### Modified Capabilities

- `eve-sso-auth`: callback emits audit events; emission ordering w.r.t. main-promotion is constrained.
- `account-management`: existing mutating endpoints (`DELETE /api/v1/account`, `POST /api/v1/characters/:id/set-main`, `DELETE /api/v1/characters/:id`) gain audit-emission requirements.
- `api-authentication`: API-key create/revoke endpoints gain audit-emission requirements.

## Impact

- **Backend**:
  - New migration `00000000000005_create_audit_log.sql` creating the `audit_log` table with three indexes (`occurred_at DESC`, partial on `actor_account_id`, partial on `actor_character_id`).
  - New module `backend/src/audit/mod.rs` containing `AuditEvent` (enum of ~28 variants with `event_type()` and `details()` methods), `ActingCharacter` struct, `record_in_tx`, `list_audit_log`, `AuditLogEntry`. Unit tests live alongside the module.
  - New `backend/src/db/characters.rs::get_main_for_account_tx(tx, account_id) -> Option<(i64, String)>` used by the audit code to snapshot the actor character. (The existing `is_main` helper returns a tuple keyed by internal UUID; this one is keyed by `eve_character_id` + name.)
  - Service-layer changes: `services/auth.rs` (SSO callback), `services/account.rs` (delete-account, set-main, delete-character), `services/api_keys.rs` (create/revoke) each open or extend the relevant transaction to include the audit call. Where a handler currently passes a `&PgPool` to a service that does a single write, the service signature widens to take or own a transaction so the audit row commits with the state change.
  - Tests: per-`AuditEvent`-variant serialization unit tests (mirroring `zz-ref/backend/older-iteration/src/audit.rs` test layout); a new integration test file `backend/tests/audit_log.rs` asserting per-emission point (registration, orphan claim, reactivation, bootstrap promotion, deletion request, set-main, character removed, api key created, api key revoked) writes exactly the expected `audit_log` rows with the correct actor columns and `details` shape, AND that no unexpected event types appear (catches accidental over-logging).
  - `backend/src/lib.rs` exports the new `audit` module.
  - `backend/.sqlx/` cache regenerated for the new `sqlx::query!` invocations.

- **Frontend**: no changes in this change. Audit-browser UI ships with the `add-server-admin-and-block-list` change that follows.

- **Out of scope**:
  - The admin-facing read endpoint (`GET /api/v1/admin/audit`) and its frontend page — they live in the next change.
  - Activating the dormant variants (`account_purged`, `map_*`, `acl_*`, blocked-character events, server-admin-revoked, admin-override events). They are intentionally present-but-silent in v1.
  - IP / User-Agent capture (deferred — no retrofit pain when added later).
  - Retention / rotation policy (audit rows accumulate forever in v1; revisit if volume warrants).
