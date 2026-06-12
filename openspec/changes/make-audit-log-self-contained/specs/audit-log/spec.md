## MODIFIED Requirements

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

#### Scenario: Ownership-change events snapshot old and new owner names

- **GIVEN** an `AuditEvent::AdminAclOwnershipChanged` from owner account A (main "Wasp 222") to owner account B (main "Wasp 223")
- **WHEN** the row is written
- **THEN** `details` contains `old_owner_name = "Wasp 222"` and `new_owner_name = "Wasp 223"` alongside their account ids

#### Scenario: Attach/detach events name both the ACL and the map

- **GIVEN** an `AuditEvent::AclAttachedToMap` attaching ACL "test1" to map "Home Chain"
- **WHEN** the row is written
- **THEN** `target_name` is "test1" (the ACL target) and `details` contains `map_name = "Home Chain"` with the map id

### Requirement: list_audit_log reads newest-first with filter axes, a name search, a time window, and a keyset cursor

The system SHALL provide an async function `audit::list_audit_log` returning `Vec<AuditLogEntry>` where each entry carries `id`, `occurred_at`, `actor_account_id`, `actor_character_id`, `actor_character_name`, `event_type`, `details`, `target_type`, `target_id`, and `target_name`.

The function SHALL accept the shipped optional filter axes (event_type, actor, target_type, target_id, target_name), a time window (`since`/`before`), a keyset cursor, and a limit, all bound as parameters (no string interpolation).

The combined name-search axis `q: Option<&str>` SHALL, when `Some`, restrict to rows where **any** of `actor_character_name`, `target_name`, or the textual representation of `details` contains the argument as a case-insensitive substring:

```
(actor_character_name ILIKE '%fragment%'
 OR target_name        ILIKE '%fragment%'
 OR details::text      ILIKE '%fragment%')
```

LIKE metacharacters (`%`, `_`, `\`) in the argument SHALL be escaped so they match literally, using the same escaping as the other substring axes. `q` and `target_name` MAY both be supplied; they combine conjunctively. The `details::text` match is a deliberate flat substring search appropriate at wormhole scale: matching a term that appears in a non-name `details` value (e.g. `permission`, `source`) is acceptable behaviour. No new column, index, or Postgres extension is required; the substring axes operate over the rows surviving the time bound, which the window keeps small.

Results SHALL be ordered by `occurred_at DESC`. When multiple axes are supplied they combine conjunctively (AND).

#### Scenario: q matches a name snapshotted only in details

- **GIVEN** an `acl_member_added` row whose `details.member_name` is "Wasp 222" and whose actor and target_name are other values
- **WHEN** `list_audit_log` is called with `q = Some("wasp 222")`
- **THEN** the row is returned (matched via `details::text`)

#### Scenario: q still matches the actor or target name

- **WHEN** `list_audit_log` is called with `q = Some("wasp")`
- **THEN** rows where either `actor_character_name` or `target_name` contains "wasp" (case-insensitive substring) are returned, as before

#### Scenario: q is a case-insensitive substring across all three sources

- **GIVEN** a row whose `details` contains the value "Admin Dude"
- **WHEN** `list_audit_log` is called with `q = Some("admin")`
- **THEN** the row is returned (case-insensitive, unanchored)

#### Scenario: q LIKE metacharacters are treated literally

- **WHEN** `list_audit_log` is called with `q = Some("%")`
- **THEN** only rows actually containing a literal "%" in actor name, target name, or details text are returned (the `%` is escaped, not treated as a wildcard)

#### Scenario: q combines conjunctively with other axes

- **WHEN** `list_audit_log` is called with `q = Some("wasp 222")`, `event_type = Some("acl_member_added")`, and `since = Some(T)`
- **THEN** only rows that match the search (across actor/target/details) AND the event type AND fall within the time window are returned
