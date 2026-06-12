# Fix Transactional Integrity

## Why

A backend review (2026-06-11) found a cluster of write paths whose invariants hold only in the absence of concurrency or partial failure: account soft-delete deletes sessions *after* its transaction commits (a failed session delete leaves a soft-deleted account with working logins for up to 7 days, and the cookie-auth path deliberately checks no account status); the last-admin and last-character guards for `DELETE /api/v1/account` and `DELETE /api/v1/characters/:id` are evaluated outside the write transaction (concurrent requests can race past them); ACL member mutations commit on the pool and then write their audit row in a *separate* transaction (a failed audit leaves an unaudited mutation — every other flow in the codebase commits both atomically); and `acl_member` has no uniqueness constraint, so duplicate members are insertable — while the service code already maps a unique-violation error that can never occur.

## What Changes

- Move session deletion into the soft-delete transaction in `services/account::delete_account` (sessions live in Postgres; the block flow already does this).
- Evaluate the last-admin guard (account delete) and the last-character / is-main guards (character delete) inside the write transaction, mirroring how `revoke_admin` already runs its guard.
- Fold each ACL member mutation (`add_member`, `update_member_permission`, `remove_member`) and its audit emission into one transaction; same for the ownership check + write in `rename_acl`/`delete_acl` where currently split.
- Add partial unique indexes on `acl_member`: one per identity shape — `(acl_id, character_id)` for character members and `(acl_id, member_type, eve_entity_id)` for corporation/alliance members. Duplicate adds become HTTP 409.
- Replace the message-substring CHECK-violation detection in `services/acl.rs` with Postgres error-code matching (`23514 check_violation`) via `sqlx::Error::Database`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `account-management`: the soft-delete requirement's atomicity expands to include session deletion; the delete-account and delete-character requirements gain race-free guard semantics.
- `acls`: ACL member identity becomes unique within an ACL (duplicate add → 409); ACL mutations and their audit events commit atomically.

## Impact

- Backend: `services/account.rs`, `services/acl.rs`, `handlers/api/v1/account.rs` (session deletion moves out of the handler), `db/accounts.rs` / `db/characters.rs` (tx-scoped guard queries), `db/acl_member.rs` (tx variants), `error.rs` (new `ConflictKind::DuplicateAclMember`), new migration for the unique indexes.
- Migration must handle pre-existing duplicate `acl_member` rows (dedupe keeps the oldest).
- Frontend: the ACL member picker surfaces the new 409 `duplicate_acl_member` code (message only — no structural change).
- Tests: sqlx + integration tests for each race/atomicity scenario; HURL for the 409.
