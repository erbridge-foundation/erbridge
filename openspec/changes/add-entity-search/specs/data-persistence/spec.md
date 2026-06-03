## ADDED Requirements

### Requirement: Orphan characters may be minted from entity search

Entity search SHALL be a flow that mints an **orphan** `eve_character` row (in addition to the existing map-ACL pre-claim flow). When an entity search matches a character that has no `eve_character` row, the system SHALL insert an orphan row — `account_id = NULL`, NULL token columns (`encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`), `scopes = '{}'`, `is_main = false` — with `name`, `corporation_id`, `corporation_name`, `alliance_id`, and `alliance_name` populated from ESI public info at mint time.

Such an orphan row SHALL be a valid, referenceable identity despite belonging to no account: its `id` UUID MAY be referenced by an `acl_member.character_id`, and it SHALL be claimable by the existing orphan-claim flow on that pilot's next SSO login (which sets `account_id` and writes tokens without creating a second row).

#### Scenario: Entity search mints an orphan for an unknown character

- **WHEN** entity search matches a character with no existing `eve_character` row
- **THEN** an orphan row is inserted with `account_id = NULL`, NULL token columns, empty `scopes`, `is_main = false`, and public-info columns populated from ESI

#### Scenario: Minted orphan is referenceable and claimable

- **WHEN** an orphan minted by entity search is later referenced as an `acl_member.character_id`, and that pilot subsequently completes an SSO login
- **THEN** the ACL member reference remains valid and the orphan-claim flow sets the row's `account_id` and tokens without creating a second row

#### Scenario: Entity-search orphan holds no token material

- **WHEN** an orphan row minted by entity search is inspected
- **THEN** `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, and `account_id` are all NULL and `scopes` is the empty array `'{}'`
