## 1. Migration

- [x] 1.1 Add `backend/migrations/00000000000006_add_audit_log_target_columns.sql` adding `target_type TEXT`, `target_id TEXT`, `target_name TEXT` (all nullable) to `audit_log`.
- [x] 1.2 In the same migration, create `audit_log_target_id_idx ON audit_log (target_type, target_id) WHERE target_id IS NOT NULL`.
- [x] 1.3 In the same migration, create `audit_log_target_name_idx ON audit_log (LOWER(target_name)) WHERE target_name IS NOT NULL`.
- [x] 1.4 In the same migration, backfill existing rows: `UPDATE audit_log SET target_type/target_id` derived from `details` + `event_type` (character events → `'character'` + `details->>'eve_character_id'`; account/key/admin events → `'account'` + `details->>'account_id'` or `actor_account_id`; map/acl events → their ids). Best-effort `target_name` via join to the target account's current main where recoverable, else leave NULL.
- [x] 1.5 Apply migration against the dev DB and confirm columns + indexes exist (`\d audit_log`).

## 2. AuditEvent::target()

- [x] 2.1 Add `AuditTarget { target_type: &'static str, target_id: String, name: AuditTargetName }` and the `AuditTargetName` enum (carried-name / account-main-lookup / none) to `backend/src/audit/mod.rs`.
- [x] 2.2 Implement `AuditEvent::target(&self) -> Option<AuditTarget>` with one match arm per variant, following the per-variant mapping in the spec. No wildcard arm (exhaustiveness enforces coverage of future variants).
- [x] 2.3 Unit-test `target()` for every variant: assert `target_type`, `target_id`, and name disposition (carried name value / account-lookup marker / none) — mirroring the existing per-variant `event_type()`/`details()` tests.

## 3. record_in_tx target population

- [x] 3.1 Extend `record_in_tx` to compute target columns from `event.target()`: NULL all three when `None`; set `target_type`/`target_id` from `Some(t)`.
- [x] 3.2 Resolve `target_name`: carried name written as-is; account-name disposition triggers `characters::get_main_for_account_tx` against the **target** account id; reuse the actor lookup when actor account == target account.
- [x] 3.3 Make the account-target name lookup fail-soft: on miss, `tracing::error!` with the target account id + event_type, write `target_name = NULL`, continue `Ok(())`.
- [x] 3.4 Extend the `INSERT` to write the three new columns; regenerate `.sqlx/` (`cargo sqlx prepare -- --all-targets`).
- [x] 3.5 Add `target_type`/`target_id`/`target_name` to the `AuditLogEntry` struct.

## 4. record_in_tx tests

- [x] 4.1 `#[sqlx::test]`: character-targeted event writes `target_type='character'`, `target_id`, carried `target_name`.
- [x] 4.2 `#[sqlx::test]`: account-targeted event with a **different** actor snapshots the **target** account's main into `target_name` (not the actor's).
- [x] 4.3 `#[sqlx::test]`: account-targeted event where the target account has no main → `tracing::error!` captured, `target_name` NULL, row still inserted (reuse the existing tracing-capture pattern).
- [x] 4.4 `#[sqlx::test]`: nameless character target (e.g. `EveCharacterBlocked`) → `target_type`/`target_id` set, `target_name` NULL.

## 5. list_audit_log target filters

- [x] 5.1 Widen `list_audit_log` signature with optional `target_type`, `target_id`, `target_name` parameters; update all call sites.
- [x] 5.2 Add the three filter clauses (parameterised; `target_name` matches `LOWER(target_name) = LOWER($n)` against the expression index) and select the three new columns into `AuditLogEntry`.
- [x] 5.3 `#[sqlx::test]`: filter by `target_type`+`target_id` returns only that entity's rows.
- [x] 5.4 `#[sqlx::test]`: filter by `target_name` is case-insensitive.
- [x] 5.5 `#[sqlx::test]`: target filters combine (AND) with `event_type`/`actor` filters.

## 6. Integration alignment

- [x] 6.1 Update `backend/tests/audit_log.rs` assertions that read back rows to also assert the new `target_*` columns for the events they exercise (registration, key, set-main, delete-character, delete-account paths).

## 7. Verification

- [x] 7.1 `cargo sqlx prepare -- --all-targets` is clean (no uncommitted query drift).
- [x] 7.2 `cargo clippy --all-targets -- -D warnings` passes.
- [x] 7.3 `cargo test` (backend) passes — unit + `#[sqlx::test]` + `tests/audit_log.rs`.
- [x] 7.4 `openspec validate add-audit-log-target-columns --strict` passes.

## 8. Downstream sequencing

- [x] 8.1 Update `add-server-admin-and-block-list`'s `audit-log` delta and its admin audit-browser spec/tasks to consume `target_type`/`target_id`/`target_name` filters instead of re-deriving target search from JSONB (do not implement that change here — only realign its not-yet-applied artifacts).
