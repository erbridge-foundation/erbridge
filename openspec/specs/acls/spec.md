## Purpose

Reusable, named access-control lists (ACLs) and their members. An ACL is owned by the account that creates it and may be attached to zero or more maps via `map_acl`. Each member grants one permission (`read`/`read_write`/`manage`/`admin`/`deny`) to a character, corporation, or alliance; `manage`/`admin` are reserved for character members. Provides the `acl` and `acl_member` tables, account-scoped CRUD, and the "manageable by this account" listing (owner or a character member holding manage/admin).

## Requirements

### Requirement: ACL is a named, account-owned, reusable access list

The system SHALL provide an `acl` resource: a named list owned by the account that created it (`owner_account_id`). An ACL exists independently of any map and MAY be attached to zero or more maps. An account SHALL be able to create an ACL, rename it, and delete it.

When an ACL is deleted, its members and all map attachments SHALL be removed (FK `ON DELETE CASCADE` on `acl_member` and `map_acl`).

#### Scenario: Create an ACL

- **WHEN** an authenticated account creates an ACL with a name
- **THEN** an `acl` row is inserted with `owner_account_id` set to that account and the given name, and the created ACL is returned

#### Scenario: Rename an ACL

- **WHEN** the owning account renames an ACL it owns
- **THEN** the ACL's `name` is updated and the updated ACL is returned

#### Scenario: Delete an ACL cascades its members and attachments

- **WHEN** the owning account deletes an ACL
- **THEN** the `acl` row is removed along with every `acl_member` row and every `map_acl` row referencing it

### Requirement: An account lists the ACLs it can manage

The system SHALL list the ACLs an account can manage: those it owns, plus those on which it holds `manage` or `admin` permission via a direct character member entry (a character on the account). The listing SHALL be ordered by name.

#### Scenario: Owner sees their ACL

- **WHEN** an account lists manageable ACLs
- **THEN** every ACL it owns appears in the result

#### Scenario: Manager sees a managed ACL

- **WHEN** an account has a character that is a `character` member of an ACL with `manage` or `admin` permission
- **THEN** that ACL appears in the account's manageable-ACLs listing even though the account does not own it

#### Scenario: Unrelated ACL is not listed

- **WHEN** an account neither owns an ACL nor holds manage/admin on it via a character member
- **THEN** that ACL does not appear in the account's manageable-ACLs listing

### Requirement: ACL members grant a permission to a character, corporation, or alliance

An `acl_member` SHALL record one grant: a `member_type` of `character`, `corporation`, or `alliance`, and a `permission` of `read`, `read_write`, `manage`, `admin`, or `deny`. A `character` member SHALL reference an `eve_character` row via `character_id`; a `corporation` or `alliance` member SHALL reference the entity via `eve_entity_id`. Each member MAY carry a denormalized `name` snapshot for display.

The system SHALL be able to add a member to an ACL, list an ACL's members, update a member's permission, and remove a member.

#### Scenario: Add a character member

- **WHEN** a manager adds a `character` member to an ACL with a permission
- **THEN** an `acl_member` row is inserted with `member_type = 'character'`, `character_id` set, and the given permission

#### Scenario: Add a corporation member

- **WHEN** a manager adds a `corporation` member to an ACL with permission `read`
- **THEN** an `acl_member` row is inserted with `member_type = 'corporation'`, `eve_entity_id` set to the corporation id, and `permission = 'read'`

#### Scenario: Update a member's permission

- **WHEN** a manager updates an existing member's permission
- **THEN** that member's `permission` is changed and the rest of the row is unaffected

#### Scenario: Remove a member

- **WHEN** a manager removes a member from an ACL
- **THEN** the `acl_member` row is deleted

### Requirement: Member type and permission are constrained

The system SHALL constrain `acl_member` so that:

- `member_type` is one of `character`, `corporation`, `alliance`;
- `permission` is one of `read`, `read_write`, `manage`, `admin`, `deny`;
- `manage` and `admin` permissions are reserved for `character` members — a `corporation` or `alliance` member MAY only hold `read`, `read_write`, or `deny`.

These constraints SHALL be enforced at the database level (CHECK constraints) in addition to any service-layer validation.

#### Scenario: Corporation member cannot be granted manage

- **WHEN** an attempt is made to add a `corporation` member with permission `manage`
- **THEN** the operation is rejected (the role-for-type constraint forbids manage/admin on non-character members)

#### Scenario: Invalid permission value is rejected

- **WHEN** an attempt is made to add a member with a permission outside the allowed set
- **THEN** the operation is rejected by the permission CHECK constraint

### Requirement: ACL member identity is unique within an ACL

An ACL SHALL NOT contain two members with the same identity: at most one `character` member per `character_id`, and at most one `corporation` or `alliance` member per (`member_type`, `eve_entity_id`). The constraint SHALL be enforced at the database level (partial unique indexes) in addition to any service-layer handling. An attempt to add a duplicate member SHALL be rejected with HTTP 409 and `error.code = "duplicate_acl_member"`.

Existing duplicate rows at migration time SHALL be deduplicated keeping the oldest row.

#### Scenario: Duplicate character member is rejected
- **WHEN** an ACL already has a `character` member for character C and a manager attempts to add C again (with any permission)
- **THEN** the response is HTTP 409 with `error.code = "duplicate_acl_member"` and no row is inserted

#### Scenario: Duplicate corporation member is rejected
- **WHEN** an ACL already has a `corporation` member for entity E and a manager attempts to add E as a `corporation` member again
- **THEN** the response is HTTP 409 with `error.code = "duplicate_acl_member"` and no row is inserted

#### Scenario: Same entity id may appear under different member types
- **WHEN** an ACL has an `alliance` member with `eve_entity_id = N`
- **THEN** adding a `corporation` member with `eve_entity_id = N` is permitted (the identities differ by member type)

#### Scenario: Migration dedupes existing duplicates
- **WHEN** the unique-index migration runs against a database containing duplicate members
- **THEN** for each duplicate set only the oldest row survives, and the indexes are created successfully

### Requirement: ACL mutations commit atomically with their audit events

Every ACL mutation that emits an audit event — create, rename, delete, member add, member permission change, member removal — SHALL perform the mutation and write its audit row in a single database transaction: the mutation MUST NOT be observable without its audit row, nor the audit row without its mutation. Ownership authorisation for the mutation SHALL be evaluated within that same transaction.

#### Scenario: Failed audit write rolls back the member add
- **WHEN** a member insert succeeds but the audit-row insert in the same transaction fails
- **THEN** the transaction rolls back and the ACL's member list is unchanged

#### Scenario: Member mutations are not split across transactions
- **WHEN** a member's permission is updated and the database fails between the update and the audit write
- **THEN** the permission change is rolled back along with the audit row — the two never diverge

### Requirement: A single manageable ACL is readable by id

`GET /api/v1/acls/{acl_id}` SHALL return the single ACL identified by `acl_id` when the caller can manage it under the same predicate as the manageable-ACLs listing (owner, or `manage`/`admin` permission via a direct character member). When the ACL does not exist, or exists but is not manageable by the caller, the response SHALL be 404 — existence is not revealed to accounts the listing would hide it from.

#### Scenario: Owner reads their ACL by id

- **WHEN** an account that owns an ACL calls `GET /api/v1/acls/{acl_id}`
- **THEN** the response is 200 with that ACL in the standard envelope

#### Scenario: Manager reads a managed ACL by id

- **WHEN** an account holding `manage` via a character member calls `GET /api/v1/acls/{acl_id}`
- **THEN** the response is 200 with that ACL

#### Scenario: Unrelated caller gets 404, not 403

- **WHEN** an account that neither owns nor manages the ACL calls `GET /api/v1/acls/{acl_id}`
- **THEN** the response is 404 (indistinguishable from a nonexistent id)

#### Scenario: Unknown id is 404

- **WHEN** `acl_id` matches no ACL
- **THEN** the response is 404
