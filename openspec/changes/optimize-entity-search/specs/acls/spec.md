# acls — delta for optimize-entity-search

## MODIFIED Requirements

### Requirement: ACL members grant a permission to a character, corporation, or alliance

An `acl_member` SHALL record one grant: a `member_type` of `character`, `corporation`, or `alliance`, and a `permission` of `read`, `read_write`, `manage`, `admin`, or `deny`. A `character` member SHALL reference an `eve_character` row via `character_id`; a `corporation` or `alliance` member SHALL reference the entity via `eve_entity_id`. Each member MAY carry a denormalized `name` snapshot for display.

The system SHALL be able to add a member to an ACL, list an ACL's members, update a member's permission, and remove a member.

When adding a `character` member, the request SHALL identify the character by exactly one of:

- `character_id` — the `eve_character.id` UUID of an existing row; or
- `eve_character_id` — the numeric EVE character id, for a character with no local row yet. In this case the system SHALL find-or-mint the orphan `eve_character` row (per the `data-persistence` orphan-mint requirement) and insert the member referencing it, within the add operation. Any ESI public-info lookups needed for the mint SHALL happen before the write transaction opens.

Supplying both identifiers, or neither, SHALL be rejected as a bad request. A concurrent mint or login-claim of the same character SHALL NOT produce a duplicate `eve_character` row (the unique `eve_character_id` index arbitrates; the loser re-reads and proceeds).

#### Scenario: Add a character member

- **WHEN** a manager adds a `character` member to an ACL with a permission
- **THEN** an `acl_member` row is inserted with `member_type = 'character'`, `character_id` set, and the given permission

#### Scenario: Add a character member by eve_character_id mints the orphan

- **WHEN** a manager adds a `character` member identified by `eve_character_id` for a character with no `eve_character` row
- **THEN** an orphan `eve_character` row is minted and the `acl_member` row references its UUID, in one add operation

#### Scenario: Add by eve_character_id reuses an existing row

- **WHEN** a manager adds a `character` member identified by `eve_character_id` for a character that already has a row (owned or orphan)
- **THEN** no new `eve_character` row is created and the member references the existing UUID

#### Scenario: Both or neither character identifier is rejected

- **WHEN** an add-member request for a `character` member carries both `character_id` and `eve_character_id`, or neither
- **THEN** the request is rejected as a bad request and nothing is inserted

#### Scenario: Add a corporation member

- **WHEN** a manager adds a `corporation` member to an ACL with permission `read`
- **THEN** an `acl_member` row is inserted with `member_type = 'corporation'`, `eve_entity_id` set to the corporation id, and `permission = 'read'`

#### Scenario: Update a member's permission

- **WHEN** a manager updates an existing member's permission
- **THEN** that member's `permission` is changed and the rest of the row is unaffected

#### Scenario: Remove a member

- **WHEN** a manager removes a member from an ACL
- **THEN** the `acl_member` row is deleted
