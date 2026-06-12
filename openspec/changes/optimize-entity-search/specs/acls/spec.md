# acls — delta for optimize-entity-search

## MODIFIED Requirements

### Requirement: ACL members grant a permission to a character, corporation, or alliance

An `acl_member` SHALL record one grant: a `member_type` of `character`, `corporation`, or `alliance`, and a `permission` of `read`, `read_write`, `manage`, `admin`, or `deny`. Every member SHALL carry `eve_entity_id` — the durable EVE id (character/corporation/alliance), the uniform external identity snapshotted into the audit event. A `character` member SHALL additionally reference an `eve_character` row via `character_id` (the internal FK link used for cascade-delete); a `corporation` or `alliance` member SHALL NOT carry `character_id`. Each member MAY carry a denormalized `name` snapshot for display.

The system SHALL be able to add a member to an ACL, list an ACL's members, update a member's permission, and remove a member.

A member add request SHALL carry `eve_entity_id` for every member type. When adding a `character` member, the request additionally distinguishes two cases by the presence of `character_id`:

- `character_id` present — the `eve_character.id` UUID of an existing row (the picker already held it from a search that matched a local row). The member is inserted referencing that UUID directly.
- `character_id` absent — the character has no local `eve_character` row yet. The system SHALL find-or-mint the orphan `eve_character` row keyed by `eve_entity_id` (per the `data-persistence` orphan-mint requirement) and insert the member referencing it, within the add operation. Any ESI public-info lookups needed for the mint SHALL happen before the write transaction opens.

A `corporation` or `alliance` member carrying `character_id` SHALL be rejected as a bad request. A member add omitting `eve_entity_id` SHALL be rejected as a bad request. A concurrent mint or login-claim of the same character SHALL NOT produce a duplicate `eve_character` row (the unique `eve_character_id` index arbitrates; the loser re-reads and proceeds).

#### Scenario: Add an existing character member by character_id

- **WHEN** a manager adds a `character` member to an ACL with `eve_entity_id`, `character_id`, and a permission
- **THEN** an `acl_member` row is inserted with `member_type = 'character'`, `character_id` set to the given UUID, `eve_entity_id` set, and the given permission; no `eve_character` row is created

#### Scenario: Add a character member without character_id mints the orphan

- **WHEN** a manager adds a `character` member with `eve_entity_id` and no `character_id`, for a character with no `eve_character` row
- **THEN** an orphan `eve_character` row is minted (keyed by `eve_entity_id`) and the `acl_member` row references its UUID, in one add operation

#### Scenario: Add without character_id reuses an existing row

- **WHEN** a manager adds a `character` member with `eve_entity_id` and no `character_id`, for a character that already has a row (owned or orphan)
- **THEN** no new `eve_character` row is created and the member references the existing UUID

#### Scenario: A member add without eve_entity_id is rejected

- **WHEN** an add-member request omits `eve_entity_id`
- **THEN** the request is rejected as a bad request and nothing is inserted

#### Scenario: Add a corporation member

- **WHEN** a manager adds a `corporation` member to an ACL with permission `read`
- **THEN** an `acl_member` row is inserted with `member_type = 'corporation'`, `eve_entity_id` set to the corporation id, no `character_id`, and `permission = 'read'`

#### Scenario: A corporation or alliance member carrying character_id is rejected

- **WHEN** an add-member request for a `corporation` or `alliance` member carries a `character_id`
- **THEN** the request is rejected as a bad request and nothing is inserted

#### Scenario: Update a member's permission

- **WHEN** a manager updates an existing member's permission
- **THEN** that member's `permission` is changed and the rest of the row is unaffected

#### Scenario: Remove a member

- **WHEN** a manager removes a member from an ACL
- **THEN** the `acl_member` row is deleted
