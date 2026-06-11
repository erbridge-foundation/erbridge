# maps — delta for add-map-acl-resource-endpoints

## ADDED Requirements

### Requirement: A single readable map is resolvable by slug

`GET /api/v1/maps/by-slug/{slug}` SHALL return the single active map with that slug when the caller holds at least `read` effective permission on it, in the same shape as the map listing (including attached-ACL summaries). An unknown slug, a soft-deleted map, and a map the caller cannot read SHALL all yield 404.

#### Scenario: Reader resolves a map by slug

- **WHEN** an account with `read` (or higher) effective permission calls `GET /api/v1/maps/by-slug/{slug}` for an active map
- **THEN** the response is 200 with that map (and its attached-ACL summaries) in the standard envelope

#### Scenario: Unreadable map is 404

- **WHEN** an account with no effective permission on the map calls `GET /api/v1/maps/by-slug/{slug}`
- **THEN** the response is 404

#### Scenario: Soft-deleted or unknown slug is 404

- **WHEN** the slug matches no map, or matches a map with `status = 'soft_deleted'`
- **THEN** the response is 404

### Requirement: Map creation can mint and attach a default ACL atomically

`POST /api/v1/maps` SHALL accept an optional `default_acl: bool`. When true, the backend SHALL — in the same transaction as the map insert — create an ACL named after the map and owned by the caller, add the caller's main character as an explicit `admin` member when the account has a main (an account without a main gets an empty ACL; the owner retains implicit admin via the resolver), attach the ACL to the new map, and emit the corresponding `acl_created`, `acl_member_added` (when seeded), `acl_attached_to_map`, and `map_created` audit events. If any step fails, the whole transaction SHALL roll back — no orphan ACL may survive a failed map creation.

Supplying both `default_acl: true` and `acl_id` SHALL be rejected as a bad request.

#### Scenario: Default-ACL creation is all-or-nothing

- **WHEN** `POST /api/v1/maps` is called with `default_acl: true` and the map insert fails (e.g. slug conflict 409)
- **THEN** no ACL row, member row, or attachment exists afterwards — the transaction rolled back

#### Scenario: Default ACL is created, seeded, and attached

- **WHEN** an account with a main character creates a map with `default_acl: true`
- **THEN** one ACL named after the map exists, owned by the caller, with the main as an `admin` character member, attached to the new map; all four audit events share the transaction

#### Scenario: No main character yields an empty default ACL

- **WHEN** an account without a main character creates a map with `default_acl: true`
- **THEN** the ACL is created and attached with no members, and no `acl_member_added` event is emitted

#### Scenario: default_acl and acl_id are mutually exclusive

- **WHEN** `POST /api/v1/maps` carries both `default_acl: true` and an `acl_id`
- **THEN** the response is 400 and nothing is created
