# data-persistence — delta for optimize-entity-search

## REMOVED Requirements

### Requirement: Orphan characters may be minted from entity search

**Reason**: Mint-on-search creates permanent rows for every result a user merely *sees*; the mint point moves to the ACL member add so only selected entities are persisted. The orphan row shape and its claim/referenceability guarantees are preserved under the replacement requirement below.

**Migration**: Rows already minted by past searches remain valid orphan identities; no data migration. Code paths needing a referenceable UUID for an unknown character go through the ACL member add (which mints) instead of the search.

## ADDED Requirements

### Requirement: Orphan characters may be minted when adding an ACL member

Adding an ACL `character` member by its `eve_entity_id` (the EVE character id) with no `character_id` SHALL be a flow that mints an **orphan** `eve_character` row (in addition to the existing map-ACL pre-claim flow). When the referenced character has no `eve_character` row, the system SHALL insert an orphan row — `account_id = NULL`, NULL token columns (`encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`), `scopes = '{}'`, `is_main = false` — with `name` taken from the add request's snapshot and `corporation_id`, `corporation_name`, `alliance_id`, `alliance_name` populated from ESI public info at mint time (falling back to placeholder values when public-info is unavailable, so the add still succeeds).

Such an orphan row SHALL be a valid, referenceable identity despite belonging to no account: its `id` UUID MAY be referenced by an `acl_member.character_id`, and it SHALL be claimable by the existing orphan-claim flow on that pilot's next SSO login (which sets `account_id` and writes tokens without creating a second row).

Entity search SHALL NOT mint orphan rows.

#### Scenario: Member add mints an orphan for an unknown character

- **WHEN** an ACL member add carries an `eve_entity_id` and no `character_id`, for a character with no existing `eve_character` row
- **THEN** an orphan row is inserted with `account_id = NULL`, NULL token columns, empty `scopes`, `is_main = false`, and public-info columns populated (or placeholders on public-info failure)

#### Scenario: Minted orphan is referenceable and claimable

- **WHEN** an orphan minted by a member add is later referenced as an `acl_member.character_id`, and that pilot subsequently completes an SSO login
- **THEN** the ACL member reference remains valid and the orphan-claim flow sets the row's `account_id` and tokens without creating a second row

#### Scenario: Member-add orphan holds no token material

- **WHEN** an orphan row minted by a member add is inspected
- **THEN** `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, and `account_id` are all NULL and `scopes` is the empty array `'{}'`
