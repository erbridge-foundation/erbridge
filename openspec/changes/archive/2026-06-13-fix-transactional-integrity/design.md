# Design — fix-transactional-integrity

## Context

Four related defects, all "works unless something races or fails mid-flight":

1. `DELETE /api/v1/account`: the service transaction (soft-delete + token clear + audit) commits, then the handler calls `session_store.remove_all_for_account` separately. The cookie-path `AuthenticatedAccount` extractor deliberately does not check `account.status` (its enforcement model is "soft-delete/block delete the sessions"), so a failed second step leaves live sessions on a soft-deleted account. `block_character` already deletes sessions inside its transaction — the model is right, the soft-delete path just doesn't follow it.
2. The last-admin guard in `delete_account` and the count/is-main guards in `delete_character` run pool-side before the transaction opens. Two concurrent character deletes on a two-character account can both pass `count > 1` and delete both rows. `revoke_admin` already shows the correct pattern (guard inside the tx, after the flip, rollback on violation).
3. `services/acl.rs` member mutations write on the pool, then open a fresh tx solely for the audit row. Mutation and audit can diverge.
4. `acl_member` has no identity uniqueness; `map_member_db_err` maps `UniqueViolation` → "duplicate acl member" but the constraint backing it was never created. Duplicates are also semantically confusing for the resolver (same grant counted twice; a duplicate with a different permission silently participates in most-permissive-wins).

## Goals / Non-Goals

**Goals:**
- Every invariant guard runs inside the transaction whose write it guards.
- Mutation + audit is one commit, everywhere, with no exceptions to reason about.
- Duplicate ACL members are impossible at the schema level and surface as a typed 409.

**Non-Goals:**
- Changing the cookie-auth extractor to check account status per request (the session-teardown enforcement model is kept; this change makes it actually hold).
- Serializable isolation or advisory locks. Read Committed plus guard-after-write-inside-tx is sufficient for these invariants (the guard re-reads state that includes the transaction's own pending writes; competing transactions serialise on the row locks the writes take).
- ACL-side audit *content* changes — only the commit boundary moves.

## Decisions

**Session deletion moves into `services/account::delete_account`'s tx.** `db/sessions.rs` already has `delete_for_account_in_tx`. The handler keeps cookie-clearing and `RefreshedJwtSlot::suppress()` (response concerns); the service owns all state mutation. The `session-store` abstraction is bypassed in favour of the db function — consistent with `block_character`.

**Guard pattern: write first, count inside, rollback on violation.** For `delete_character`: open tx → delete the row (`WHERE id = $1 AND account_id = $2`, also subsuming the ownership check) → re-check invariants inside the tx (`SELECT count(*)`, `is_main` of the deleted row captured via `RETURNING`) → rollback + 409 if violated. For `delete_account`'s last-admin guard: count active admins inside the tx after the status flip (`count_server_admins_tx` already exists for revoke). Concurrent deletes serialise on the row lock; the second transaction sees the first's committed delete and its guard fails. Alternative considered: `SELECT … FOR UPDATE` pre-locks — equivalent outcome, more code.

**ACL: ownership check moves inside the same tx as the write.** `load_owned_acl` currently reads pool-side, then the mutation runs later (TOCTOU on ownership transfer — minor today since ownership never changes, but free to fix while restructuring). Each member mutation becomes: open tx → assert ownership (tx-scoped read) → write → audit → commit. db functions gain tx variants where missing; where a pool variant has no remaining caller it is removed rather than kept alongside.

**Uniqueness via two partial unique indexes.**
```sql
CREATE UNIQUE INDEX acl_member_unique_character
    ON acl_member (acl_id, character_id) WHERE member_type = 'character';
CREATE UNIQUE INDEX acl_member_unique_entity
    ON acl_member (acl_id, member_type, eve_entity_id) WHERE member_type <> 'character';
```
Partial indexes match the two identity shapes without a synthetic combined column. The migration first dedupes existing rows keeping the oldest (`DELETE … USING` self-join on `ctid`/`created_at`), inside the same migration file so the index creation cannot fail on dirty data. Duplicate add maps `DbError::UniqueViolation` → new `ConflictKind::DuplicateAclMember` (409 `duplicate_acl_member`) — replacing the current 400 mapping, since "already a member" is a conflict, not a malformed request.

**CHECK-violation detection by SQLSTATE.** `map_member_db_err` matches `db_err.code() == "23514"` instead of substring-matching the message. The constraint *name* (`db_err.constraint()`) further distinguishes role-for-type from permission-set violations if a finer message is wanted; the response stays 400 for CHECK violations (the request itself was invalid).

## Risks / Trade-offs

- [Existing duplicate members in production data] The dedupe keeps the oldest row; if duplicates carry *different* permissions, the surviving permission may differ from what a user last set. → Acceptable: duplicates are a bug state; the audit log retains the history. Migration logs how many rows it removed.
- [409 vs current 400 for duplicates] Technically a contract change for an error path the schema previously never triggered. → The frontend picker treats both as a displayed error message; HURL updated.
- [Wider transactions hold locks slightly longer] All are single-row or per-account row sets; no long-running work moves inside a tx (no ESI calls). Negligible.

## Migration Plan

One migration (dedupe + two indexes). Deploy backend; frontend message-key addition rides along. Rollback: revert code; the indexes are compatible with reverted code (the old code simply never triggers them — except duplicate adds fail as 400 via the old `UniqueViolation` mapping, which is acceptable during a rollback window).
