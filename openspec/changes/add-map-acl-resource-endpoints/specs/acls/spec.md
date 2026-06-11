# acls — delta for add-map-acl-resource-endpoints

## ADDED Requirements

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
