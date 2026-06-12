# acls — delta for fix-transactional-integrity

## ADDED Requirements

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
