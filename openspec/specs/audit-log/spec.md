## Purpose

Append-only audit trail of state-changing actions in the backend. Provides the `audit_log` Postgres table, the `AuditEvent` Rust enum that catalogues every recordable action, the `record_in_tx` write helper that participates in the caller's transaction, and the `list_audit_log` keyset-paginated read helper. Audit rows snapshot both the acting account (when one exists) and the acting account's main character (EVE ID + name at write time) so attribution survives account hard-deletes and character renames.
## Requirements
### Requirement: audit_log table schema

The system SHALL provide an `audit_log` table with the following columns:

- `id` — `UUID PRIMARY KEY DEFAULT gen_random_uuid()`
- `occurred_at` — `TIMESTAMPTZ NOT NULL DEFAULT now()`
- `actor_account_id` — `UUID REFERENCES account(id) ON DELETE SET NULL`
- `actor_character_id` — `BIGINT` (EVE character ID, snapshot at write time, no FK)
- `actor_character_name` — `TEXT` (snapshot at write time, no FK)
- `event_type` — `TEXT NOT NULL`
- `details` — `JSONB NOT NULL DEFAULT '{}'`
- `target_type` — `TEXT` (nullable; the kind of entity the action targeted, e.g. `'character'`, `'account'`, `'map'`, `'acl'`)
- `target_id` — `TEXT` (nullable; the stringified identifier of the target — an EVE character ID for `'character'` targets, a UUID for `'account'` / `'map'` / `'acl'` targets; no FK, snapshot)
- `target_name` — `TEXT` (nullable; the human-readable name of the target at write time — the character name for character targets, the target account's main character name for account targets; snapshot, no FK)

The `target_*` columns are nullable because not every recordable action has a distinct target.

The table SHALL have the following indexes:

- `audit_log_occurred_at_idx ON audit_log (occurred_at DESC)` — newest-first reads.
- `audit_log_actor_account_idx ON audit_log (actor_account_id) WHERE actor_account_id IS NOT NULL` — per-actor filters; partial because rows with NULL actor (system events) are not filtered by this axis.
- `audit_log_actor_character_idx ON audit_log (actor_character_id) WHERE actor_character_id IS NOT NULL` — per-character history queries; partial for the same reason.
- `audit_log_target_id_idx ON audit_log (target_type, target_id) WHERE target_id IS NOT NULL` — "all events against this entity" queries; partial because target-less rows do not participate.
- `audit_log_target_name_idx ON audit_log (LOWER(target_name)) WHERE target_name IS NOT NULL` — backs case-insensitive target-name search (`LOWER(target_name) = LOWER($1)` or prefix `LIKE`); partial for the same reason.

The `actor_account_id` foreign key SHALL use `ON DELETE SET NULL` so that historical audit rows survive a future hard-delete of an account row. The actor-character columns and all three `target_*` columns SHALL have no foreign key constraint; they are snapshots intended to outlive the referenced rows.

#### Scenario: Schema is created by migration
- **WHEN** the backend applies all migrations
- **THEN** the `audit_log` table exists with the ten columns and five indexes described above

#### Scenario: Actor account FK survives account row deletion
- **WHEN** an `account` row that is referenced by audit rows is hard-deleted
- **THEN** the audit rows remain; their `actor_account_id` becomes NULL; their `actor_character_id`, `actor_character_name`, `event_type`, `details`, and `target_*` columns are unchanged

#### Scenario: Actor character columns survive eve_character row deletion
- **WHEN** the `eve_character` row identified by an audit row's `actor_character_id` is hard-deleted
- **THEN** the audit row remains; its `actor_character_id` and `actor_character_name` are unchanged (no FK cascade)

#### Scenario: Target columns survive deletion of the referenced entity
- **GIVEN** an audit row with `target_type`, `target_id`, and `target_name` populated
- **WHEN** the entity identified by `target_id` (account, character, map, or acl) is later hard-deleted
- **THEN** the audit row's `target_type`, `target_id`, and `target_name` are unchanged (no FK cascade; they are snapshots)

#### Scenario: Existing rows are backfilled by the migration
- **GIVEN** `audit_log` rows that existed before this change (with `target_*` columns absent)
- **WHEN** the migration that adds the `target_*` columns is applied
- **THEN** each pre-existing row whose event has a derivable target has `target_type` and `target_id` populated from its `details`; `target_name` is populated where the target's name is recoverable at migration time and left NULL otherwise

### Requirement: audit_log is INSERT-only

The application code SHALL define no path that issues `UPDATE` or `DELETE` against the `audit_log` table. The admin-facing read interface (introduced in a subsequent change) SHALL be read-only — no edit, no delete, no "fix typo" affordance is permitted to exist. The denormalised `actor_character_name` column relies on this invariant to preserve the name as it was at the time of the event.

This invariant SHALL be enforced by code review and spec discipline, not by a database trigger.

#### Scenario: No UPDATE path exists
- **WHEN** the backend codebase is searched for SQL targeting `audit_log`
- **THEN** every match is either `INSERT` (in the audit module) or `SELECT` (in the audit module's read helper); no `UPDATE` or `DELETE` is present

### Requirement: AuditEvent enum is the catalogue of recordable actions

The system SHALL provide a Rust `AuditEvent` enum in `backend/src/audit/mod.rs` that enumerates every recordable action. Each variant SHALL carry the typed data needed to render the per-event JSON payload. The enum SHALL expose three methods:

- `event_type(&self) -> &'static str` — returns the snake_case identifier used in the `audit_log.event_type` column.
- `details(&self) -> serde_json::Value` — returns the per-event JSON payload written to `audit_log.details`.
- `target(&self) -> Option<AuditTarget>` — returns the entity the action targeted, or `None` for events with no distinct target.

`AuditTarget` SHALL be a struct exposing the target's kind, stringified id, and an optional resolved name (as defined by the shipped capability). `AuditTargetName` SHALL distinguish a known name, an account whose name is resolved at write time as the target account's main character name, and no name.

The catalogue SHALL be defined in full, including variants for features that do not yet emit any rows, so `event_type` strings stay stable. Every variant SHALL return its correct `target()`.

**Self-contained naming principle.** Every entity an event references SHALL carry a snapshotted human-readable **name** captured at write time, so the row stays readable after the entity is deleted. Names SHALL NOT be resolved at read time. The primary entity's name SHALL be carried in `target_name`; the name of any *secondary* entity (one the `target_*` columns cannot hold) SHALL be carried in `details`. For ESI-resolved entities (character/corp/alliance), the **EVE id** SHALL be stored alongside the name; the internal `acl_member` row UUID SHALL NOT be stored. Keys in `details` that merely duplicate `target_id` SHALL be removed.

The per-variant target mapping SHALL be:

- `AccountRegistered`, `AccountReactivated`, `AccountPurged`, `AccountDeletionRequested` → target_type `"account"`, target_id the affected `account_id`, name resolved from the account's main.
- `OrphanCharacterClaimed`, `CharacterAdded` → target_type `"character"`, target_id the `eve_character_id`, name the carried `character_name`.
- `CharacterRemoved`, `CharacterSetMain` → target_type `"character"`, target_id the `eve_character_id`, name the carried `character_name`.
- `ApiKeyCreated`, `ApiKeyRevoked` → target_type `"account"`, target_id the `account_id`, name resolved from the account's main.
- `ServerAdminGranted`, `ServerAdminRevoked` → target_type `"account"`, target_id the affected `account_id`, name resolved from the account's main.
- `EveCharacterBlocked`, `EveCharacterUnblocked`, `BlockedLoginRejected` → target_type `"character"`, target_id the `eve_character_id`, name the carried `character_name` (or NULL where the subject character name is genuinely unavailable at the SSO emit site).
- `CharacterAddRejectedBoundElsewhere` → target_type `"character"`, target_id the `eve_character_id`, name NULL (the rejected character is the subject; its name is not carried into `target_name`).
- `MapCreated`, `MapDeleted`, `AdminMapHardDeleted` → target_type `"map"`, target_id the `map_id`, name the carried map `name`.
- `AdminMapOwnershipChanged` → target_type `"map"`, target_id the `map_id`, name the carried map `name`.
- `AclCreated`, `AclRenamed`, `AclDeleted`, `AdminAclHardDeleted` → target_type `"acl"`, target_id the `acl_id`, name the carried acl `name` (for `AclRenamed`, the `new_name`).
- `AclMemberAdded`, `AclMemberPermissionChanged`, `AclMemberRemoved`, `AclAttachedToMap`, `AclDetachedFromMap`, `AdminAclOwnershipChanged` → target_type `"acl"`, target_id the `acl_id`, name the carried acl `name`.

The `details()` payloads SHALL be:

- `AclMemberAdded` → `{ member_name, member_type, permission, eve_entity_id }` (the `eve_entity_id` is the member's durable EVE id for every member type — character, corporation, or alliance. Character members now carry their EVE character id in `acl_member.eve_entity_id` too, supplied by the picker at add time alongside the internal `character_id` FK link, so the audit snapshot is uniform and needs no read-time lookup; `acl_id` removed as it duplicates `target_id`; the internal `member_id` removed).
- `AclMemberPermissionChanged` → `{ member_name, permission, eve_entity_id }`.
- `AclMemberRemoved` → `{ member_name, eve_entity_id }`.
- `AclAttachedToMap`, `AclDetachedFromMap` → `{ map_name, map_id }` (the ACL is the target and is named via `target_name`; the map is the secondary entity, carried with id + name — the ACL is not duplicated in `details`).
- `AdminMapOwnershipChanged` → `{ old_owner_name, old_owner, new_owner_name, new_owner }`.
- `AdminAclOwnershipChanged` → `{ old_owner_name, old_owner, new_owner_name, new_owner }`.
- `ApiKeyRevoked` → `{ key_name }` (key label snapshotted; `key_id` retained for correlation is OPTIONAL).
- `CharacterRemoved`, `CharacterSetMain`, `EveCharacterBlocked`, `EveCharacterUnblocked`, `BlockedLoginRejected`, `CharacterOwnerMismatch` → carry the character name (in `target_name` and/or `details.character_name`) wherever the emit site can supply it.
- `CharacterAddRejectedBoundElsewhere` → `{ eve_character_id }` only. Like `BlockedLoginRejected` it records a rejected *attempt* rather than a committed state change, but here an authenticated session exists, so `actor_account_id` SHALL be the session account that attempted the add. The owning account SHALL NOT be recorded in `details` — an audit reader with DB access can resolve it, but the event must not casually leak account linkage into the admin audit browser.

All other variants retain their shipped `details()` shapes.

#### Scenario: AclMemberAdded snapshots the member name and EVE id, not the internal UUID

- **GIVEN** an `AuditEvent::AclMemberAdded` for a character member named "Wasp 222" with EVE id 95465499 added with permission "admin"
- **WHEN** the row is written
- **THEN** `target_type = "acl"` and `target_name` is the ACL's name; `details` contains `member_name = "Wasp 222"`, `eve_entity_id = 95465499`, `member_type`, and `permission`; `details` does NOT contain `acl_id` (it duplicates `target_id`) nor the internal `member_id`

#### Scenario: AclMemberRemoved row remains readable after the member row is deleted

- **GIVEN** an `acl_member_removed` row whose `details.member_name` was snapshotted at write time
- **WHEN** the underlying `acl_member` row has since been deleted
- **THEN** the audit row still names the removed member via `details.member_name` (no join required, no read-time resolution)

#### Scenario: Character-subject events carry a target name

- **GIVEN** an `AuditEvent::CharacterRemoved` whose emit site holds the character name
- **WHEN** `target()` is called and the row is written
- **THEN** `target_type = "character"`, `target_id = eve_character_id.to_string()`, and `target_name` is the character's name (not NULL)

#### Scenario: Rejected add is recorded with the session actor

- **WHEN** the add-character flow is refused because the character is bound to another account
- **THEN** an audit row exists with `event_type = "character_add_rejected_bound_elsewhere"`, `actor_account_id` = the session account, `target_type = "character"`, and `target_id` = the character's EVE id

#### Scenario: The owning account is not leaked in details

- **WHEN** the `character_add_rejected_bound_elsewhere` row is inspected via the admin audit browser
- **THEN** `details` contains the `eve_character_id` but not the other account's id

#### Scenario: Ownership-change events snapshot old and new owner names

- **GIVEN** an `AuditEvent::AdminAclOwnershipChanged` from owner account A (main "Wasp 222") to owner account B (main "Wasp 223")
- **WHEN** the row is written
- **THEN** `details` contains `old_owner_name = "Wasp 222"` and `new_owner_name = "Wasp 223"` alongside their account ids

#### Scenario: Attach/detach events name both the ACL and the map

- **GIVEN** an `AuditEvent::AclAttachedToMap` attaching ACL "test1" to map "Home Chain"
- **WHEN** the row is written
- **THEN** `target_name` is "test1" (the ACL target) and `details` contains `map_name = "Home Chain"` with the map id

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

The function SHALL INSERT a single row into `audit_log` with `event_type = event.event_type()`, `details = event.details()`, and the `target_type` / `target_id` / `target_name` columns populated from `event.target()`. The write SHALL execute against the passed transaction so it commits with — or rolls back with — the state change in the same transaction.

Actor-column resolution SHALL follow these rules, in order:

1. If `actor_account_id` is `Some(id)`, the function SHALL look up the account's main character (the unique `eve_character` row with `account_id = id AND is_main = TRUE`) within the same transaction. If found, `actor_character_id` and `actor_character_name` SHALL be populated from that row's `eve_character_id` and `name`. The looked-up `id` SHALL be written to `actor_account_id`.
2. Else if `acting_as` is `Some(c)`, the function SHALL write NULL to `actor_account_id`, `c.eve_character_id` to `actor_character_id`, and `c.name` to `actor_character_name`.
3. Else (both `None`), the function SHALL write NULL to all three actor columns.

Target-column resolution SHALL follow from `event.target()`:

1. If `target()` is `None`, all three target columns SHALL be NULL.
2. If `target()` is `Some(t)`, `target_type` SHALL be `t.target_type` and `target_id` SHALL be `t.target_id`.
3. `target_name` SHALL be populated according to `t`'s name disposition:
   - a directly-carried name SHALL be written as-is;
   - an account-name disposition SHALL trigger a lookup of the **target account's** main character within the same transaction; the main's name SHALL be snapshotted into `target_name`. If the lookup returns no row (invariant violation), the function SHALL emit a `tracing::error!` identifying the missing main and the event_type, write `target_name = NULL`, and continue (fail-soft — identical discipline to the actor snapshot);
   - no name SHALL leave `target_name` NULL.

The main-character lookup for the actor and for an account target MAY be the same query when the actor account and the target account are the same id; the function MAY reuse a single lookup in that case.

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
- **WHEN** `record_in_tx(tx, None, Some(ActingCharacter { eve_character_id: 99999, name: "Signing In" }), event)` is called
- **THEN** the inserted row has `actor_account_id = NULL`, `actor_character_id = 99999`, `actor_character_name = "Signing In"`

#### Scenario: System events leave all actor columns NULL
- **WHEN** `record_in_tx(tx, None, None, event)` is called
- **THEN** the inserted row has `actor_account_id = NULL`, `actor_character_id = NULL`, `actor_character_name = NULL`

#### Scenario: Main-character lookup miss falls soft with tracing error
- **GIVEN** an account whose main-character lookup unexpectedly returns no row
- **WHEN** `record_in_tx(tx, Some(account_id), None, event)` is called
- **THEN** the function emits a `tracing::error!` describing the missing main and the event_type, the inserted row has `actor_account_id = account_id` but `actor_character_id = NULL` and `actor_character_name = NULL`, and the function returns `Ok(())`

#### Scenario: Character-target columns are written from the event
- **GIVEN** an `AuditEvent::CharacterAdded { account_id, eve_character_id: 555, character_name: "Alt Pilot" }`
- **WHEN** `record_in_tx(tx, Some(account_id), None, event)` is called
- **THEN** the inserted row has `target_type = "character"`, `target_id = "555"`, and `target_name = "Alt Pilot"`

#### Scenario: Account-target name snapshots the target account's main
- **GIVEN** a target account whose main character is (eve_character_id 222, name "Boss Pilot")
- **WHEN** `record_in_tx` is called for an `AuditEvent::ServerAdminGranted { account_id: <target>, source }` (with the actor being a different admin account)
- **THEN** the inserted row has `target_type = "account"`, `target_id = "<target uuid>"`, and `target_name = "Boss Pilot"` (snapshotted from the target account's main, independent of the actor)

#### Scenario: Account-target name lookup miss falls soft
- **GIVEN** a target account that has no main character (invariant violation)
- **WHEN** `record_in_tx` is called for an account-targeted event against that account
- **THEN** the function emits a `tracing::error!` identifying the missing target main and the event_type, the inserted row has `target_type = "account"` and `target_id` set but `target_name = NULL`, and the function returns `Ok(())`

#### Scenario: Target-less disposition leaves target columns NULL
- **GIVEN** an event whose `target()` returns `None`
- **WHEN** `record_in_tx` is called
- **THEN** the inserted row has `target_type = NULL`, `target_id = NULL`, and `target_name = NULL`

### Requirement: list_audit_log reads newest-first with filter axes, a name search, a time window, and a keyset cursor

The system SHALL provide an async function `audit::list_audit_log` returning `Vec<AuditLogEntry>` where each `AuditLogEntry` carries `id`, `occurred_at`, `actor_account_id`, `actor_character_id`, `actor_character_name`, `event_type`, `details`, `target_type`, `target_id`, and `target_name`.

The function SHALL accept these optional filter axes plus a name search, a time window, a keyset cursor, and a limit, all bound as parameters (no string interpolation, no SQL injection surface):

- `event_type: Option<&str>` — if `Some`, restricts to rows where `event_type` matches; if `None`, no restriction.
- `actor_account_id: Option<Uuid>` — if `Some`, restricts to rows where `actor_account_id` matches; if `None`, no restriction.
- `target_type: Option<&str>` — if `Some`, restricts to rows where `target_type` matches; if `None`, no restriction.
- `target_id: Option<&str>` — if `Some`, restricts to rows where `target_id` matches; if `None`, no restriction.
- `target_name: Option<&str>` — if `Some`, restricts to rows where `target_name` contains the argument as a case-insensitive substring (`target_name ILIKE '%fragment%'`); LIKE metacharacters (`%`, `_`, `\`) in the argument SHALL be escaped so they match literally. If `None`, no restriction.
- `q: Option<&str>` — the combined name-search axis. If `Some`, restricts to rows where **any** of `actor_character_name`, `target_name`, or the textual representation of `details` contains the argument as a case-insensitive substring:

  ```
  (actor_character_name ILIKE '%fragment%'
   OR target_name        ILIKE '%fragment%'
   OR details::text      ILIKE '%fragment%')
  ```

  LIKE metacharacters (`%`, `_`, `\`) in the argument SHALL be escaped so they match literally, using the same escaping as the other substring axes. If `None`, no restriction. `q` and `target_name` MAY both be supplied; they then combine conjunctively (a row must satisfy both). The `details::text` match is a deliberate flat substring search appropriate at wormhole scale: matching a term that appears in a non-name `details` value (e.g. `permission`, `source`) is acceptable behaviour.
- `since: Option<DateTime<Utc>>` — if `Some`, restricts to rows where `occurred_at >= since` (the lower bound of the time window); if `None`, no lower restriction.
- `before: Option<DateTime<Utc>>` — if `Some`, restricts to rows where `occurred_at < before` (keyset cursor for newest-first pagination, and the exclusive upper bound of the time window); if `None`, no upper restriction.
- `limit: i64` — maximum rows to return; the caller is responsible for clamping to a sensible upper bound.

Results SHALL be ordered by `occurred_at DESC` (newest first). When multiple axes are supplied they SHALL be combined conjunctively (AND). The `since` lower bound SHALL use the existing `audit_log_occurred_at_idx` for a bounded range scan; the substring axes (`q`, `target_name`, including the `q` match against `details::text`) operate over the rows surviving the time bound and other equality filters, which the time window keeps small. No new column, index, or Postgres extension is required.

#### Scenario: List with no filters returns rows newest-first
- **GIVEN** several `audit_log` rows
- **WHEN** `list_audit_log` is called with all axes `None` and a limit
- **THEN** all rows are returned in descending `occurred_at` order, capped at the `limit`, each carrying its `target_*` columns

#### Scenario: Filter by target_id restricts to that entity's events
- **GIVEN** audit rows targeting several entities
- **WHEN** `list_audit_log` is called with `target_type = Some("character")` and `target_id = Some("555")`
- **THEN** only rows whose `target_type = "character"` and `target_id = "555"` are returned

#### Scenario: q matches either the actor name or the target name
- **GIVEN** an audit row whose `actor_character_name = "Wasp 223"` (target unrelated), and another row whose `target_name = "Red Wasp Industries"` (actor unrelated)
- **WHEN** `list_audit_log` is called with `q = Some("wasp")`
- **THEN** both rows are returned (the first matched on actor name, the second on a substring of the target name)

#### Scenario: q is a case-insensitive substring, not anchored
- **GIVEN** an audit row with `target_name = "The Wasp"`
- **WHEN** `list_audit_log` is called with `q = Some("wasp")`
- **THEN** the row is returned (the fragment matches mid-name, not only as a prefix)

#### Scenario: q matches a name snapshotted only in details
- **GIVEN** an `acl_member_added` row whose `details.member_name` is "Wasp 222" and whose actor and target_name are other values
- **WHEN** `list_audit_log` is called with `q = Some("wasp 222")`
- **THEN** the row is returned (matched via `details::text`)

#### Scenario: q is a case-insensitive substring across all three sources
- **GIVEN** a row whose `details` contains the value "Admin Dude"
- **WHEN** `list_audit_log` is called with `q = Some("admin")`
- **THEN** the row is returned (case-insensitive, unanchored)

#### Scenario: q LIKE metacharacters are treated literally
- **WHEN** `list_audit_log` is called with `q = Some("%")`
- **THEN** only rows actually containing a literal "%" in actor name, target name, or details text are returned (the `%` is escaped, not treated as a wildcard)

#### Scenario: since bounds the lower edge of the time window
- **GIVEN** audit rows spanning several months
- **WHEN** `list_audit_log` is called with `since = Some(T)` and `before = None`
- **THEN** only rows with `occurred_at >= T` are returned, newest-first

#### Scenario: since and before together express a bounded window
- **GIVEN** audit rows spanning several months
- **WHEN** `list_audit_log` is called with `since = Some(T_lo)` and `before = Some(T_hi)` where `T_lo < T_hi`
- **THEN** only rows with `T_lo <= occurred_at < T_hi` are returned, supporting keyset pagination *within* a fixed window (each page narrows `before`, `since` stays fixed)

#### Scenario: q combines conjunctively with other axes
- **WHEN** `list_audit_log` is called with `q = Some("wasp 222")`, `event_type = Some("acl_member_added")`, and `since = Some(T)`
- **THEN** only rows that match the search (across actor/target/details) AND the event type AND fall within the time window are returned

#### Scenario: Filter by target_name is a case-insensitive substring match
- **GIVEN** an audit row with `target_name = "Wasp 223"`
- **WHEN** `list_audit_log` is called with `target_name = Some("wasp")`
- **THEN** that row is returned (a lowercased fragment matches a substring of the name)

#### Scenario: before cursor advances pagination
- **GIVEN** a previous page's oldest `occurred_at = T`
- **WHEN** `list_audit_log` is called with `before = Some(T)`
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

