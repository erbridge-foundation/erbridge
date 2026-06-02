## ADDED Requirements

### Requirement: A map is an account-owned, soft-deletable container

The system SHALL provide a `map` resource with a `name`, a globally-unique `slug`, an optional `description`, and an `owner_account_id` referencing the creating account (`ON DELETE SET NULL`). A map SHALL carry a soft-delete lifecycle mirroring the account convention: a `status` defaulting to `active` and a nullable `delete_requested_at`. The reference subsystem's checkpoint and retention columns are NOT part of this resource.

An authenticated account SHALL be able to create a map, get a map it can read, update a map's name/slug/description, and soft-delete a map.

#### Scenario: Create a map

- **WHEN** an authenticated account creates a map with a name and slug
- **THEN** a `map` row is inserted with `owner_account_id` set to that account, `status = 'active'`, and the created map is returned

#### Scenario: Slug must be unique

- **WHEN** an account creates a map whose slug is already taken by an existing map
- **THEN** the operation fails with a slug-conflict error and no row is inserted

#### Scenario: Soft-delete a map

- **WHEN** an authorized account deletes a map
- **THEN** the map's `status` is set to a deleted state and `delete_requested_at` is recorded, rather than the row being physically removed

#### Scenario: A soft-deleted map is excluded from access resolution

- **WHEN** access to a map whose `status` is not `active` is resolved for any account
- **THEN** the owner bypass does not apply (the owner check requires `status = 'active'`)

### Requirement: Effective permission on a map is resolved from ownership and attached ACLs

The system SHALL resolve an account's **effective permission** on a map as follows:

1. If the account owns the map (and the map's `status = 'active'`), the effective permission is `admin`.
2. Otherwise, the system SHALL collect every permission granted to the account across all ACLs attached to the map, matching the account's characters by direct character membership (`acl_member.character_id`), corporation membership (`acl_member.eve_entity_id = character.corporation_id`), or alliance membership (`acl_member.eve_entity_id = character.alliance_id`, non-null).
3. If any matched member carries `deny`, the effective permission is **none** (a hard stop overriding all grants).
4. Otherwise, the effective permission is the **most-permissive** matched grant, ordered `read < read_write < manage < admin`.
5. If no member matches, the effective permission is **none**.

The permission ordering SHALL satisfy `admin > manage > read_write > read`.

#### Scenario: Owner gets admin

- **WHEN** the owning account's effective permission on its active map is resolved
- **THEN** the result is `admin`, regardless of any ACL entries

#### Scenario: Corporation grant resolves to its permission

- **WHEN** a non-owner account has a character whose corporation is a `corporation` member with `read_write` on an ACL attached to the map, and no `deny` matches
- **THEN** the account's effective permission is `read_write`

#### Scenario: Most-permissive grant wins

- **WHEN** an account matches multiple members across attached ACLs granting `read` and `manage` with no `deny`
- **THEN** the effective permission is `manage`

#### Scenario: Deny overrides all grants

- **WHEN** an account matches members granting `admin` and also a member carrying `deny`
- **THEN** the effective permission is none and access is refused

#### Scenario: No match means no access

- **WHEN** a non-owner account matches no member of any ACL attached to the map
- **THEN** the effective permission is none

### Requirement: Map operations are gated by effective permission

Every map operation SHALL require a minimum effective permission, refusing the request when the account's resolved permission is below it (or none):

- read a map (`GET`) — requires at least `read`;
- update a map (name/slug/description) — requires at least `manage`;
- soft-delete a map — requires `admin`;
- attach or detach an ACL — requires `admin`.

A request from an account whose effective permission is below the requirement SHALL be refused with a forbidden error, not served.

#### Scenario: Reader cannot update

- **WHEN** an account whose effective permission is `read` attempts to update a map
- **THEN** the request is refused (manage required)

#### Scenario: Non-member cannot read

- **WHEN** an account with no effective permission requests a map
- **THEN** the request is refused

#### Scenario: Admin may delete

- **WHEN** an account whose effective permission is `admin` (e.g. the owner) soft-deletes the map
- **THEN** the map is soft-deleted

### Requirement: Maps are listed for the accounts that can read them

The system SHALL list the maps an account can read — maps it owns plus maps to which it has a resolved (non-`deny`) grant via an attached ACL — and SHALL annotate each listed map with the ACLs attached to it that the account can manage.

#### Scenario: Owner sees their map in the list

- **WHEN** an account lists maps
- **THEN** every active map it owns appears in the result

#### Scenario: Granted map appears in the list

- **WHEN** an account has a resolved non-deny grant on a map via an attached ACL
- **THEN** that map appears in the account's map listing

### Requirement: An ACL is attached to a map by the map's administrator

The system SHALL allow attaching an ACL to a map and detaching it, recorded in the `map_acl` join (`PRIMARY KEY (map_id, acl_id)`, both sides `ON DELETE CASCADE`). Attaching SHALL require the caller to hold `admin` on the map AND to own the ACL being attached. An ACL MAY be attached to many maps and a map MAY have many ACLs.

#### Scenario: Owner attaches an owned ACL

- **WHEN** a map administrator attaches an ACL it owns to the map
- **THEN** a `map_acl` row linking the map and the ACL is created

#### Scenario: Cannot attach an ACL you do not own

- **WHEN** a map administrator attempts to attach an ACL it does not own
- **THEN** the operation is refused with an ACL-owner mismatch error

#### Scenario: Detach removes the link only

- **WHEN** a map administrator detaches an ACL from the map
- **THEN** the `map_acl` row is removed while the `acl` and its members remain intact

### Requirement: Map and ACL-attachment mutations are audited

The system SHALL emit audit events, through the existing audit log, when a map is created, a map is soft-deleted, an ACL is attached to a map, and an ACL is detached from a map.

#### Scenario: Map creation is audited

- **WHEN** an account creates a map
- **THEN** an audit event recording the actor account and the map is written

#### Scenario: ACL attachment is audited

- **WHEN** an ACL is attached to a map
- **THEN** an audit event recording the actor account, the map, and the ACL is written
