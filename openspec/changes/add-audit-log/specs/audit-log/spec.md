## ADDED Requirements

### Requirement: audit_log table schema

The system SHALL provide an `audit_log` table with the following columns:

- `id` — `UUID PRIMARY KEY DEFAULT gen_random_uuid()`
- `occurred_at` — `TIMESTAMPTZ NOT NULL DEFAULT now()`
- `actor_account_id` — `UUID REFERENCES account(id) ON DELETE SET NULL`
- `actor_character_id` — `BIGINT` (EVE character ID, snapshot at write time, no FK)
- `actor_character_name` — `TEXT` (snapshot at write time, no FK)
- `event_type` — `TEXT NOT NULL`
- `details` — `JSONB NOT NULL DEFAULT '{}'`

The table SHALL have three indexes:

- `audit_log_occurred_at_idx ON audit_log (occurred_at DESC)` — newest-first reads.
- `audit_log_actor_account_idx ON audit_log (actor_account_id) WHERE actor_account_id IS NOT NULL` — per-actor filters; partial because rows with NULL actor (system events) are not filtered by this axis.
- `audit_log_actor_character_idx ON audit_log (actor_character_id) WHERE actor_character_id IS NOT NULL` — per-character history queries; partial for the same reason.

The `actor_account_id` foreign key SHALL use `ON DELETE SET NULL` so that historical audit rows survive a future hard-delete of an account row. The actor-character columns SHALL have no foreign key constraint; they are snapshots intended to outlive the referenced `eve_character` row.

#### Scenario: Schema is created by migration
- **WHEN** the backend applies all migrations
- **THEN** the `audit_log` table exists with the seven columns and three indexes described above

#### Scenario: Actor account FK survives account row deletion
- **WHEN** an `account` row that is referenced by audit rows is hard-deleted
- **THEN** the audit rows remain; their `actor_account_id` becomes NULL; their `actor_character_id`, `actor_character_name`, `event_type`, and `details` columns are unchanged

#### Scenario: Actor character columns survive eve_character row deletion
- **WHEN** the `eve_character` row identified by an audit row's `actor_character_id` is hard-deleted
- **THEN** the audit row remains; its `actor_character_id` and `actor_character_name` are unchanged (no FK cascade)

### Requirement: audit_log is INSERT-only

The application code SHALL define no path that issues `UPDATE` or `DELETE` against the `audit_log` table. The admin-facing read interface (introduced in a subsequent change) SHALL be read-only — no edit, no delete, no "fix typo" affordance is permitted to exist. The denormalised `actor_character_name` column relies on this invariant to preserve the name as it was at the time of the event.

This invariant SHALL be enforced by code review and spec discipline, not by a database trigger.

#### Scenario: No UPDATE path exists
- **WHEN** the backend codebase is searched for SQL targeting `audit_log`
- **THEN** every match is either `INSERT` (in the audit module) or `SELECT` (in the audit module's read helper); no `UPDATE` or `DELETE` is present

### Requirement: AuditEvent enum is the catalogue of recordable actions

The system SHALL provide a Rust `AuditEvent` enum in `backend/src/audit/mod.rs` that enumerates every recordable action. Each variant SHALL carry the typed data needed to render the per-event JSON payload. The enum SHALL expose two methods:

- `event_type(&self) -> &'static str` — returns the snake_case identifier used in the `audit_log.event_type` column.
- `details(&self) -> serde_json::Value` — returns the per-event JSON payload written to `audit_log.details`.

The catalogue SHALL be defined in full from the day the audit_log capability ships, including variants for features that do not yet emit any rows. This keeps `event_type` strings stable across future changes that activate currently-dormant variants.

The v1 catalogue SHALL contain at minimum the following variants. Variants marked **(emitted in v1)** are wired up by this change. Variants marked **(dormant)** are present in the enum and unit-tested for serialization shape, but no production code path emits them in v1.

- `AccountRegistered { account_id, eve_character_id, character_name }` — **(emitted in v1)**
- `AccountDeletionRequested { account_id }` — **(emitted in v1)**
- `AccountReactivated { account_id }` — **(emitted in v1)**
- `AccountPurged { account_id }` — **(dormant)**
- `CharacterAdded { account_id, eve_character_id, character_name }` — **(emitted in v1)**
- `CharacterRemoved { account_id, eve_character_id }` — **(emitted in v1)**
- `CharacterSetMain { account_id, eve_character_id }` — **(emitted in v1)**
- `OrphanCharacterClaimed { account_id, eve_character_id, character_name }` — **(emitted in v1)** (renamed from older iteration's `GhostCharacterClaimed`)
- `ApiKeyCreated { account_id, key_id, name }` — **(emitted in v1)**
- `ApiKeyRevoked { account_id, key_id }` — **(emitted in v1)**
- `ServerAdminGranted { account_id, source: ServerAdminGrantSource }` — **(partially emitted in v1)**: the `FirstAccountBootstrap` source SHALL be emitted by the SSO callback when the first account is auto-promoted. The `AdminGrant` source SHALL NOT be emitted in v1 (no admin-grant path exists yet).
- `ServerAdminRevoked { account_id }` — **(dormant)**
- `EveCharacterBlocked { eve_character_id, reason: Option<String> }` — **(dormant)**
- `EveCharacterUnblocked { eve_character_id }` — **(dormant)**
- `MapCreated`, `MapDeleted`, `AdminMapOwnershipChanged`, `AdminMapHardDeleted` — **(dormant)**
- `AclCreated`, `AclRenamed`, `AclDeleted`, `AclMemberAdded`, `AclMemberPermissionChanged`, `AclMemberRemoved`, `AclAttachedToMap`, `AclDetachedFromMap`, `AdminAclOwnershipChanged`, `AdminAclHardDeleted` — **(dormant)**

The `ServerAdminGrantSource` SHALL be its own enum with at least the variants `FirstAccountBootstrap` and `AdminGrant`, each rendered to a snake_case string by an `as_str()` accessor.

The exact JSON shape of `details()` per variant SHALL follow the rule: if the actor account column carries the affected account_id (i.e. actor is the same as the affected account), `details` SHALL NOT repeat `account_id`. If the actor is NULL (system event) or differs from the affected entity, the affected ID(s) SHALL appear in `details` so the row is self-contained.

#### Scenario: event_type returns the expected snake_case string for each variant
- **WHEN** `event.event_type()` is called for any defined variant
- **THEN** it returns the corresponding snake_case identifier (e.g. `AccountRegistered → "account_registered"`, `OrphanCharacterClaimed → "orphan_character_claimed"`, `ServerAdminGranted → "server_admin_granted"`)

#### Scenario: details() omits account_id when the actor column carries it
- **GIVEN** an `AuditEvent::CharacterAdded { account_id, eve_character_id, character_name }`
- **WHEN** `details()` is called
- **THEN** the returned JSON contains `eve_character_id` and `character_name` but NOT `account_id` (the actor column carries it)

#### Scenario: details() includes account_id when actor will be NULL
- **GIVEN** an `AuditEvent::AccountRegistered { account_id, eve_character_id, character_name }` (emitted with `actor_account_id` NULL because no session exists yet)
- **WHEN** `details()` is called
- **THEN** the returned JSON contains `account_id`, `eve_character_id`, and `character_name`

#### Scenario: ServerAdminGrantSource serialises to snake_case
- **WHEN** `ServerAdminGrantSource::FirstAccountBootstrap.as_str()` is called
- **THEN** it returns `"first_account_bootstrap"`
- **WHEN** `ServerAdminGrantSource::AdminGrant.as_str()` is called
- **THEN** it returns `"admin_grant"`

#### Scenario: OrphanCharacterClaimed replaces GhostCharacterClaimed naming
- **WHEN** an orphan character is claimed by an account
- **THEN** the emitted `event_type` SHALL be `"orphan_character_claimed"` (the older codebase's `"ghost_character_claimed"` SHALL NOT appear in any v1 emit path)

### Requirement: record_in_tx writes a single audit row participating in the caller's transaction

The system SHALL provide an async function `audit::record_in_tx` with the signature:

```rust
pub async fn record_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    actor_account_id: Option<Uuid>,
    acting_as: Option<ActingCharacter>,
    event: AuditEvent,
) -> Result<()>
```

where `ActingCharacter` is a struct `{ eve_character_id: i64, name: String }`.

The function SHALL INSERT a single row into `audit_log` with `event_type = event.event_type()` and `details = event.details()`. The write SHALL execute against the passed transaction so it commits with — or rolls back with — the state change in the same transaction.

Actor-column resolution SHALL follow these rules, in order:

1. If `actor_account_id` is `Some(id)`, the function SHALL look up the account's main character (the unique `eve_character` row with `account_id = id AND is_main = TRUE`) within the same transaction. If found, `actor_character_id` and `actor_character_name` SHALL be populated from that row's `eve_character_id` and `name`. The looked-up `id` SHALL be written to `actor_account_id`.
2. Else if `acting_as` is `Some(c)`, the function SHALL write NULL to `actor_account_id`, `c.eve_character_id` to `actor_character_id`, and `c.name` to `actor_character_name`.
3. Else (both `None`), the function SHALL write NULL to all three actor columns.

#### Scenario: Audit row commits with the state change
- **WHEN** a service starts a transaction, performs a state change, calls `record_in_tx`, and commits
- **THEN** both the state change and the audit row are visible to subsequent transactions
- **WHEN** a service starts a transaction, performs a state change, calls `record_in_tx`, and the transaction is rolled back before commit
- **THEN** neither the state change nor the audit row is visible to subsequent transactions

#### Scenario: actor_account_id resolves the main character snapshot
- **GIVEN** an account with a main character (eve_character_id 12345, name "Test Pilot")
- **WHEN** `record_in_tx(tx, Some(account_id), None, event)` is called
- **THEN** the inserted row has `actor_account_id = account_id`, `actor_character_id = 12345`, `actor_character_name = "Test Pilot"`

#### Scenario: acting_as path used when no session exists yet
- **WHEN** `record_in_tx(tx, None, Some(ActingCharacter { eve_character_id: 99999, name: "Signing In" }), event)` is called (the SSO-callback path before main is set, or for events emitted without an authenticated session)
- **THEN** the inserted row has `actor_account_id = NULL`, `actor_character_id = 99999`, `actor_character_name = "Signing In"`

#### Scenario: System events leave all actor columns NULL
- **WHEN** `record_in_tx(tx, None, None, event)` is called (system action, e.g. a future purge sweep)
- **THEN** the inserted row has `actor_account_id = NULL`, `actor_character_id = NULL`, `actor_character_name = NULL`

#### Scenario: Main-character lookup miss falls soft with tracing error
- **GIVEN** an account whose main-character lookup unexpectedly returns no row (invariant violation, e.g. audit emitted before the SSO callback's `promote_if_no_main` step ran)
- **WHEN** `record_in_tx(tx, Some(account_id), None, event)` is called
- **THEN** the function emits a `tracing::error!` describing the missing main and the event_type, the inserted row has `actor_account_id = account_id` but `actor_character_id = NULL` and `actor_character_name = NULL`, and the function returns `Ok(())` (the state change is allowed to commit)

### Requirement: list_audit_log reads newest-first with three filter axes and a keyset cursor

The system SHALL provide an async function `audit::list_audit_log(pool, event_type, actor_account_id, before, limit)` returning `Vec<AuditLogEntry>` where each `AuditLogEntry` carries `id`, `occurred_at`, `actor_account_id`, `actor_character_id`, `actor_character_name`, `event_type`, and `details`.

The query SHALL filter using bound parameters (no string interpolation, no SQL injection surface) with the semantics:

- `event_type: Option<&str>` — if `Some`, restricts to rows where `event_type = $1`; if `None`, no restriction.
- `actor_account_id: Option<Uuid>` — if `Some`, restricts to rows where `actor_account_id = $2`; if `None`, no restriction.
- `before: Option<DateTime<Utc>>` — if `Some`, restricts to rows where `occurred_at < $3` (keyset cursor for newest-first pagination); if `None`, no restriction.
- `limit: i64` — maximum rows to return; the caller is responsible for clamping to a sensible upper bound.

Results SHALL be ordered by `occurred_at DESC` (newest first).

#### Scenario: List with no filters returns rows newest-first
- **GIVEN** several `audit_log` rows
- **WHEN** `list_audit_log(pool, None, None, None, 100)` is called
- **THEN** all rows are returned in descending `occurred_at` order, capped at the `limit`

#### Scenario: Filter by event_type restricts results
- **GIVEN** mixed audit rows
- **WHEN** `list_audit_log(pool, Some("account_registered"), None, None, 100)` is called
- **THEN** only rows with `event_type = "account_registered"` are returned

#### Scenario: Filter by actor_account_id restricts results
- **GIVEN** audit rows from multiple actors
- **WHEN** `list_audit_log(pool, None, Some(account_a), None, 100)` is called
- **THEN** only rows whose `actor_account_id = account_a` are returned; rows with NULL actor are excluded

#### Scenario: before cursor advances pagination
- **GIVEN** a previous page's oldest `occurred_at = T`
- **WHEN** `list_audit_log(pool, None, None, Some(T), 100)` is called
- **THEN** only rows with `occurred_at < T` are returned, supporting stable keyset pagination under concurrent inserts

### Requirement: Actor-character snapshot is the account's main at write time

Whenever an audit row is written with non-NULL `actor_account_id`, the `actor_character_id` and `actor_character_name` columns SHALL reflect the EVE character ID and name of that account's main character at the moment of the `record_in_tx` call. The values SHALL NOT be recomputed at read time; they SHALL remain frozen even if the main character is later changed or renamed.

This SHALL hold despite future events that change the main: a `character_set_main` audit row emitted at time `T2` SHALL snapshot the previous main (which was the main at `T2`, before the change committed within the same transaction), and rows written after `T2` SHALL snapshot the new main.

#### Scenario: Snapshot reflects main at write time, not current state
- **GIVEN** an account whose main was character A at the time event X was audited
- **WHEN** the account later promotes character B to main and other events are audited
- **THEN** the audit row for X still shows A's `actor_character_id` and `actor_character_name`; the audit rows for later events show B's

#### Scenario: Renaming a character does not rewrite past audit rows
- **GIVEN** an audit row written with `actor_character_name = "Old Name"` at time T
- **WHEN** the underlying `eve_character.name` is later updated to "New Name"
- **THEN** the audit row's `actor_character_name` is still "Old Name"

#### Scenario: Main-history is recoverable from audit log
- **GIVEN** an account that has had characters A, B, C as main in that order
- **WHEN** the audit log is queried for that account's `character_set_main` events ordered by `occurred_at`
- **THEN** the sequence of `eve_character_id` values in `details` (combined with the initial `account_registered` or first `character_added` row) is sufficient to reconstruct which character was the main at any past point in time
