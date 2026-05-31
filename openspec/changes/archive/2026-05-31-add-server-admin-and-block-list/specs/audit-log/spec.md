## MODIFIED Requirements

### Requirement: AuditEvent enum is the catalogue of recordable actions

The system SHALL provide a Rust `AuditEvent` enum in `backend/src/audit/mod.rs` that enumerates every recordable action. Each variant SHALL carry the typed data needed to render the per-event JSON payload. The enum SHALL expose two methods:

- `event_type(&self) -> &'static str` — returns the snake_case identifier used in the `audit_log.event_type` column.
- `details(&self) -> serde_json::Value` — returns the per-event JSON payload written to `audit_log.details`.

The catalogue SHALL be defined in full, including variants for features that do not yet emit any rows. This keeps `event_type` strings stable across future changes that activate currently-dormant variants.

The catalogue SHALL contain at minimum the following variants. Variants marked **(emitted)** are wired up by some shipped change. Variants marked **(dormant)** are present in the enum and unit-tested for serialization shape, but no production code path emits them yet.

- `AccountRegistered { account_id, eve_character_id, character_name }` — **(emitted)**
- `AccountDeletionRequested { account_id }` — **(emitted)**
- `AccountReactivated { account_id }` — **(emitted)**
- `AccountPurged { account_id }` — **(dormant)**
- `CharacterAdded { account_id, eve_character_id, character_name }` — **(emitted)**
- `CharacterRemoved { account_id, eve_character_id }` — **(emitted)**
- `CharacterSetMain { account_id, eve_character_id }` — **(emitted)**
- `OrphanCharacterClaimed { account_id, eve_character_id, character_name }` — **(emitted)** (renamed from older iteration's `GhostCharacterClaimed`)
- `ApiKeyCreated { account_id, key_id, name }` — **(emitted)**
- `ApiKeyRevoked { account_id, key_id }` — **(emitted)**
- `ServerAdminGranted { account_id, source: ServerAdminGrantSource }` — **(emitted)**: the `FirstAccountBootstrap` source is emitted by the SSO callback when the first account is auto-promoted; the `AdminGrant` source is emitted by `POST /api/v1/admin/accounts/:id/grant-admin` (per the `server-administration` capability).
- `ServerAdminRevoked { account_id }` — **(emitted)**: emitted by `POST /api/v1/admin/accounts/:id/revoke-admin`.
- `EveCharacterBlocked { eve_character_id, reason: Option<String> }` — **(emitted)**: emitted by `POST /api/v1/admin/blocks`.
- `EveCharacterUnblocked { eve_character_id }` — **(emitted)**: emitted by `DELETE /api/v1/admin/blocks/:eve_character_id`.
- `BlockedLoginRejected { eve_character_id }` — **(emitted)**: emitted by the SSO callback when a blocked character is refused (per the `eve-sso-auth` capability). This variant records a rejected *attempt* rather than a committed state change — a deliberate, narrow extension of the audit log for a security-relevant event. `actor_account_id` is NULL (no account is authenticated); the `eve_character_id` is carried in `details`.
- `MapCreated`, `MapDeleted`, `AdminMapOwnershipChanged`, `AdminMapHardDeleted` — **(dormant)**
- `AclCreated`, `AclRenamed`, `AclDeleted`, `AclMemberAdded`, `AclMemberPermissionChanged`, `AclMemberRemoved`, `AclAttachedToMap`, `AclDetachedFromMap`, `AdminAclOwnershipChanged`, `AdminAclHardDeleted` — **(dormant)**

The `ServerAdminGrantSource` SHALL be its own enum with at least the variants `FirstAccountBootstrap` and `AdminGrant`, each rendered to a snake_case string by an `as_str()` accessor.

The exact JSON shape of `details()` per variant SHALL follow the rule: if the actor account column carries the affected account_id (i.e. actor is the same as the affected account), `details` SHALL NOT repeat `account_id`. If the actor is NULL (system event, or a subject-not-actor such as a blocked-login rejection) or differs from the affected entity, the affected ID(s) SHALL appear in `details` so the row is self-contained.

#### Scenario: event_type returns the expected snake_case string for each variant
- **WHEN** `event.event_type()` is called for any defined variant
- **THEN** it returns the corresponding snake_case identifier (e.g. `AccountRegistered → "account_registered"`, `OrphanCharacterClaimed → "orphan_character_claimed"`, `ServerAdminGranted → "server_admin_granted"`, `EveCharacterBlocked → "eve_character_blocked"`, `BlockedLoginRejected → "blocked_login_rejected"`)

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
- **THEN** the emitted `event_type` SHALL be `"orphan_character_claimed"` (the older codebase's `"ghost_character_claimed"` SHALL NOT appear in any emit path)

#### Scenario: BlockedLoginRejected carries the subject character in details
- **GIVEN** an `AuditEvent::BlockedLoginRejected { eve_character_id }`
- **WHEN** `details()` is called
- **THEN** the returned JSON contains `eve_character_id`; the event is emitted with `actor_account_id = NULL` (the rejected character is the subject, not an actor)
