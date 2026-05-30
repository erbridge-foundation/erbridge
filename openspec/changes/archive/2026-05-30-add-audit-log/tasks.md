# Tasks

## 1. Database migration

- [x] 1.1 Create `backend/migrations/00000000000005_create_audit_log.sql` with the `audit_log` table (seven columns: `id`, `occurred_at`, `actor_account_id` with `ON DELETE SET NULL` FK, `actor_character_id BIGINT`, `actor_character_name TEXT`, `event_type TEXT NOT NULL`, `details JSONB NOT NULL DEFAULT '{}'`) and three indexes (`occurred_at DESC` full; `actor_account_id` partial; `actor_character_id` partial).
- [x] 1.2 Apply the migration locally (`sqlx migrate run` against the dev DB or restart the dev stack); confirm the table and indexes are present (`\d audit_log` in psql).

## 2. DB helper: main-character lookup keyed by account

- [x] 2.1 Add `pub async fn get_main_for_account_tx(tx: &mut Transaction<'_, Postgres>, account_id: Uuid) -> Result<Option<(i64, String)>>` to `backend/src/db/characters.rs`. Query: `SELECT eve_character_id, name FROM eve_character WHERE account_id = $1 AND is_main = TRUE LIMIT 1`.
- [x] 2.2 Add sqlx test `get_main_for_account_tx_returns_main`: insert account + two characters (one main, one not), assert returns the main's `(eve_character_id, name)`.
- [x] 2.3 Add sqlx test `get_main_for_account_tx_returns_none_when_no_main`: insert account with no characters, assert returns `None`.

## 3. Audit module: types and enum

- [x] 3.1 Create `backend/src/audit/mod.rs`. Add `pub struct ActingCharacter { pub eve_character_id: i64, pub name: String }`. Derive `Debug`, `Clone`.
- [x] 3.2 Add `pub enum ServerAdminGrantSource { FirstAccountBootstrap, AdminGrant }` with `pub fn as_str(self) -> &'static str` returning `"first_account_bootstrap"` / `"admin_grant"`. Derive `Debug`, `Clone`, `Copy`.
- [x] 3.3 Add `pub enum AuditEvent` with all ~28 variants per the `audit-log` spec's catalogue. Use the older iteration (`zz-ref/backend/older-iteration/src/audit.rs`) as the structural reference, applying the renames: `GhostCharacterClaimed` → `OrphanCharacterClaimed`. Keep `AccountDeletionRequested` (no rename to `AccountSoftDeleted`). Derive `Debug`, `Clone`.
- [x] 3.4 Implement `impl AuditEvent { pub fn event_type(&self) -> &'static str { … } }`. Every variant returns its snake_case identifier; `OrphanCharacterClaimed` → `"orphan_character_claimed"`.
- [x] 3.5 Implement `impl AuditEvent { pub fn details(&self) -> serde_json::Value { … } }`. Per the spec rule: omit `account_id` when the actor column carries it; include affected IDs when actor is NULL or differs from the affected entity. Follow the older iteration's per-variant comments verbatim except for the rename.
- [x] 3.6 Add `pub struct AuditLogEntry { pub id: Uuid, pub occurred_at: DateTime<Utc>, pub actor_account_id: Option<Uuid>, pub actor_character_id: Option<i64>, pub actor_character_name: Option<String>, pub event_type: String, pub details: serde_json::Value }`.
- [x] 3.7 Wire the module into `backend/src/lib.rs`: `pub mod audit;`.

## 4. Audit module: unit tests for enum shape

- [x] 4.1 Add `#[cfg(test)] mod tests` to `backend/src/audit/mod.rs`. Port the unit tests from `zz-ref/backend/older-iteration/src/audit.rs` (lines 434–705), adjusting for the `OrphanCharacterClaimed` rename. Every variant SHALL have at least one test that asserts both `event_type()` and the shape of `details()`.
- [x] 4.2 Ensure unit tests for dormant variants (`AccountPurged`, all `Map*`, all `Acl*`, all `Admin*Hard*` / `Admin*Ownership*`, `EveCharacterBlocked`, `EveCharacterUnblocked`, `ServerAdminRevoked`, and the `AdminGrant` source) are present so future emission paths inherit shape coverage from day one.

## 5. Audit module: record_in_tx

- [x] 5.1 Implement `pub async fn record_in_tx(tx: &mut Transaction<'_, Postgres>, actor_account_id: Option<Uuid>, acting_as: Option<ActingCharacter>, event: AuditEvent) -> Result<()>`. Internally:
   - Compute `event_type` and `details` from the `event` argument.
   - Resolve actor character columns:
     1. If `actor_account_id.is_some()`, call `db::characters::get_main_for_account_tx(tx, …)`.
        - On `Some((eve_id, name))`, populate both character columns.
        - On `None`, emit `tracing::error!(account_id, event_type, "audit: account has no main at write time — actor character columns left NULL")` and continue with NULL character columns.
     2. Else if `acting_as.is_some()`, populate character columns from the `ActingCharacter`.
     3. Else, leave both NULL.
   - `INSERT INTO audit_log (actor_account_id, actor_character_id, actor_character_name, event_type, details) VALUES ($1, $2, $3, $4, $5)`.
- [x] 5.2 Add a sqlx test `record_in_tx_with_account_actor_snapshots_main`: insert account + main character, call `record_in_tx` with `Some(account_id)`, commit, assert the row has `actor_account_id = account_id`, `actor_character_id = <main eve id>`, `actor_character_name = <main name>`, `event_type = <expected>`.
- [x] 5.3 Add a sqlx test `record_in_tx_with_acting_as_writes_character_columns`: call `record_in_tx` with `None, Some(ActingCharacter { eve_character_id: 99999, name: "Signing In" })`, commit, assert the row has `actor_account_id = NULL`, `actor_character_id = 99999`, `actor_character_name = "Signing In"`.
- [x] 5.4 Add a sqlx test `record_in_tx_system_event_writes_all_nulls`: call `record_in_tx(tx, None, None, event)`, commit, assert all three actor columns are NULL.
- [x] 5.5 Add a sqlx test `record_in_tx_with_account_missing_main_fails_soft`: insert account with no characters, call `record_in_tx(tx, Some(account_id), None, event)`, commit, assert the row has `actor_account_id = account_id` but `actor_character_id = NULL` and `actor_character_name = NULL`, and the function returned `Ok`. (We do not assert on the `tracing::error!` here — that's covered by inspection during code review.)
- [x] 5.6 Add a sqlx test `record_in_tx_rolls_back_with_caller_tx`: open a tx, call `record_in_tx`, then deliberately roll back; assert no `audit_log` row is visible.

## 6. Audit module: list_audit_log

- [x] 6.1 Implement `pub async fn list_audit_log(pool: &PgPool, event_type: Option<&str>, actor_account_id: Option<Uuid>, before: Option<DateTime<Utc>>, limit: i64) -> Result<Vec<AuditLogEntry>>`. SQL: `SELECT id, occurred_at, actor_account_id, actor_character_id, actor_character_name, event_type, details FROM audit_log WHERE ($1::TEXT IS NULL OR event_type = $1) AND ($2::UUID IS NULL OR actor_account_id = $2) AND ($3::TIMESTAMPTZ IS NULL OR occurred_at < $3) ORDER BY occurred_at DESC LIMIT $4`.
- [x] 6.2 Add a sqlx test `list_audit_log_no_filters_returns_newest_first`: insert three rows at different `occurred_at` times (via direct INSERT with explicit timestamps), call with no filters, assert results are ordered DESC.
- [x] 6.3 Add a sqlx test `list_audit_log_filter_by_event_type`: insert mixed rows, call with `Some("account_registered")`, assert only matching rows returned.
- [x] 6.4 Add a sqlx test `list_audit_log_filter_by_actor_account_id`: insert rows for two actors, call with `Some(actor_a)`, assert only A's rows returned; rows with NULL actor are excluded.
- [x] 6.5 Add a sqlx test `list_audit_log_before_cursor`: insert rows at times T1 < T2 < T3, call with `before = Some(T3)`, assert only T1 and T2 rows returned, ordered DESC.

## 7. Retrofit: SSO callback emits audit events

- [x] 7.1 In `backend/src/services/auth.rs` (the SSO callback's completion path), inside the existing transaction and *after* `promote_if_no_main`, add audit emissions per the `eve-sso-auth` spec delta:
   - If a new account was created (first-character flow): emit `AccountRegistered { … }` with `actor_account_id = None, acting_as = Some(ActingCharacter { eve_character_id, name: character_name })`.
   - If an orphan was claimed: emit `OrphanCharacterClaimed { … }` with the same actor pattern as above.
   - If the resolved account was reactivated from soft-deleted: emit `AccountReactivated { account_id }` with the same actor pattern.
   - If the first-account bootstrap fired (the new account got `is_server_admin = TRUE`): emit `ServerAdminGranted { account_id, source: ServerAdminGrantSource::FirstAccountBootstrap }` with the same actor pattern.
   - In add-character mode: emit `CharacterAdded { … }` with `actor_account_id = Some(account_id), acting_as = None`. If the add-character flow claimed an orphan, emit `OrphanCharacterClaimed { … }` with the same actor pattern.
- [x] 7.2 Create `backend/tests/audit_log.rs` integration test file (port the layout of `zz-ref/backend/older-iteration/tests/audit_log.rs`, adapting to current code shape and event names). Add `test_first_account_registration_writes_account_registered_and_bootstrap_admin_grant`: drive the SSO completion path (via the relevant service helper, mirroring how existing tests in `backend/tests/` exercise it), assert exactly two `audit_log` rows: `account_registered` and `server_admin_granted` with `details.source = "first_account_bootstrap"`. Both rows SHALL have `actor_account_id = NULL`, `actor_character_id = <eve id>`, `actor_character_name = <name>`. Assert "no unexpected event types" appear.
- [x] 7.3 Add `test_second_account_registration_does_not_emit_bootstrap_admin_grant`: drive registration for two different EVE characters, assert the second one's audit rows do not include `server_admin_granted`.
- [x] 7.4 Add `test_orphan_claim_on_login_writes_account_registered_and_orphan_claim`: insert an orphan `eve_character` directly, drive SSO completion for that character, assert both `account_registered` and `orphan_character_claimed` rows present with the expected actor-character snapshot.
- [x] 7.5 Add `test_add_character_writes_character_added_with_main_as_actor`: register a first account (capturing the main's eve id + name), then drive the add-character flow for a second character, assert the `character_added` row has `actor_account_id = <account>`, `actor_character_id = <main's eve id>`, `actor_character_name = <main's name>`, and `details.eve_character_id = <second character's eve id>`.
- [x] 7.6 Add `test_add_character_claiming_orphan_writes_orphan_claim_with_main_actor`: register first account, insert an orphan, drive add-character flow that claims that orphan, assert `orphan_character_claimed` row with `actor_account_id = <account>` and `actor_character_*` populated from the main.
- [x] 7.7 Add `test_reactivation_writes_account_reactivated`: register account, soft-delete it directly via SQL, drive SSO completion for the same character, assert an `account_reactivated` row exists with `actor_character_*` populated from the signing-in character.

## 8. Retrofit: account-management endpoints emit audit events

- [x] 8.1 In `backend/src/services/account.rs::delete_account`, inside the existing transaction (already opened per the `clear-tokens-on-soft-delete` change), call `audit::record_in_tx(&mut tx, Some(account_id), None, AuditEvent::AccountDeletionRequested { account_id })` before commit.
- [x] 8.2 In `backend/src/services/account.rs` (or wherever the set-main service lives — locate via the existing `POST /api/v1/characters/:id/set-main` handler), wrap the existing `db::characters::set_main` call in an explicit transaction if not already; emit `AuditEvent::CharacterSetMain { account_id, eve_character_id }` *before* the `set_main` flip commits (so the actor-character snapshot reflects the outgoing main). The `eve_character_id` carried in `details` SHALL be the EVE id of the *new* main (per the spec).
- [x] 8.3 In the service backing `DELETE /api/v1/characters/:id`, open a transaction, fetch the character's `eve_character_id` (so we can carry it in details after the row is gone), emit `AuditEvent::CharacterRemoved { account_id, eve_character_id }`, perform the delete, commit. Errors short-circuit before the audit emit.
- [x] 8.4 Extend (or add) integration tests in `backend/tests/audit_log.rs`:
   - `test_delete_account_writes_account_deletion_requested`: register account, soft-delete via service, assert one `account_deletion_requested` row exists with `actor_account_id = <account>`, `actor_character_*` from the main, `details = {}`.
   - `test_set_main_writes_character_set_main_with_outgoing_main_snapshot`: register account with main A, add a second character B, call set-main(B), assert the audit row has `actor_character_id = A.eve_id`, `actor_character_name = A.name`, `details.eve_character_id = B.eve_id`. Then call set-main(A) again, assert the next row has `actor_character_id = B.eve_id` (now the outgoing) and `details.eve_character_id = A.eve_id`.
   - `test_remove_character_writes_character_removed`: register account, add a second character, remove the second character, assert one `character_removed` row exists with `details.eve_character_id` matching the removed character's EVE id.
   - `test_rejected_character_remove_writes_no_audit_row`: register account with a single character, attempt to remove it (rejected with `cannot_remove_last_character`), assert no `audit_log` row was written for the request.

## 9. Retrofit: API key endpoints emit audit events

- [x] 9.1 In `backend/src/services/api_keys.rs::create_api_key` (or equivalent), wrap the INSERT in a transaction (if not already), emit `AuditEvent::ApiKeyCreated { account_id, key_id, name }` inside that transaction, commit.
- [x] 9.2 In the same module's revoke path (the service backing `DELETE /api/v1/keys/:id`), wrap the DELETE in a transaction (if not already), emit `AuditEvent::ApiKeyRevoked { account_id, key_id }` inside that transaction, commit.
- [x] 9.3 Extend `backend/tests/audit_log.rs`:
   - `test_create_api_key_writes_api_key_created`: register account, create a key with `name = "ci"`, assert one `api_key_created` row exists with `details.key_id = <new uuid>`, `details.name = "ci"`, `actor_account_id = <account>`, `actor_character_*` from main.
   - `test_revoke_api_key_writes_api_key_revoked`: create then revoke a key, assert one `api_key_revoked` row exists with `details.key_id` matching the deleted row.
   - `test_rejected_create_writes_no_audit_row`: attempt `POST /api/v1/keys` with empty name (HTTP 400), assert no audit row.
   - `test_revoke_nonexistent_writes_no_audit_row`: attempt `DELETE /api/v1/keys/<random uuid>` (HTTP 404), assert no audit row.

## 10. Drift and tidying

- [x] 10.1 Re-run `cargo sqlx prepare -- --all-targets` from `backend/`. Commit the regenerated `.sqlx/` cache.
- [x] 10.2 Grep the codebase for stray references to "ghost_character" or `GhostCharacterClaimed` (should be none in `backend/`; the older iteration's reference under `zz-ref/` is untouched).
- [x] 10.3 Update `backend/src/lib.rs` module ordering if needed (alphabetical or per existing convention); ensure `audit` is exported alongside `db`, `dto`, `error`, etc.

## 11. Verification (backend-only — this change does not touch frontend code)

- [x] 11.1 `cargo fmt --check` from `backend/`.
- [x] 11.2 `cargo clippy --all-targets --all-features -- -D warnings` from `backend/`.
- [x] 11.3 `cargo sqlx prepare --check -- --all-targets` from `backend/` (verifies the `.sqlx/` cache matches the queries in source).
- [x] 11.4 `cargo test` from `backend/` — all unit + sqlx integration tests pass, including the new audit unit tests in `src/audit/mod.rs` and the new integration file `tests/audit_log.rs`.
- [x] 11.5 Hurl smoke against the running dev stack at `http://localhost:5000` (Traefik-mapped). `me.hurl` and `keys.hurl` ran clean (10/10 requests passing) authenticated with the user's bearer key. Live `audit_log` rows after the run show the expected emissions end-to-end: `account_registered` + `server_admin_granted{first_account_bootstrap}` from the user's original SSO (actor NULL, `acting_as` snapshot of Wasp 223 / EVE ID 679815158), plus `api_key_created` and `api_key_revoked` from the keys.hurl flow (actor populated, character snapshot non-null, key_id matching across the create/revoke pair). The destructive `account.hurl` and `characters.hurl` flows were intentionally skipped — they soft-delete the account and promote/delete characters; coverage for those audit emissions is provided by the 15 integration tests in `backend/tests/audit_log.rs`.

## 12. Wrap-up

- [x] 12.1 Run `openspec validate add-audit-log --strict` — must pass.
- [x] 12.2 Confirm the design note "audit history begins at deployment time of this change" is captured somewhere visible (currently in `design.md` Migration Plan); no extra docs needed.
- [x] 12.3 If any new memory entries are warranted (e.g. an updated `project-backend-auth-model` to reference the audit module), add them; otherwise no memory changes.
