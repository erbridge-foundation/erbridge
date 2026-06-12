# entity-search — delta for optimize-entity-search

## REMOVED Requirements

### Requirement: Character results resolve to a referenceable character UUID, minting an orphan when absent

**Reason**: Minting permanent `eve_character` rows for every search result lets any authenticated account grow the table by up to 25 placeholder rows per search, including `(0, "")`-corporation placeholders when public-info fails. The mint point moves to the ACL member add (see the `acls` capability delta), where exactly the selected entity is minted.

**Migration**: Search results for characters now carry the UUID only when a row already exists; consumers needing a referenceable UUID for an unknown character add it via `POST /api/v1/acls/{acl_id}/members` with `eve_entity_id` and no `character_id`, and the add mints the orphan at that point.

## ADDED Requirements

### Requirement: Character results carry the known character UUID and never mint rows

For every character matched by the search, the endpoint SHALL return the numeric `eve_character_id` and `name`, plus the `eve_character.id` UUID **when a row already exists** for that character (account-owned or orphan); when no row exists the UUID field SHALL be null. The search SHALL NOT write to the database: no orphan rows are minted at search time. Existing-row lookups SHALL be batched (a single query for all matched character ids), not one query per result.

#### Scenario: Existing character resolves to its UUID

- **WHEN** a matched character already has an `eve_character` row
- **THEN** the result carries that row's `id` UUID and no new row is created

#### Scenario: Unknown character carries no UUID and mints nothing

- **WHEN** a matched character has no `eve_character` row
- **THEN** the result carries `id = null` alongside its `eve_character_id` and `name`, and the database is unchanged by the search

#### Scenario: Search is write-free

- **WHEN** any entity search completes, across any categories and any result mix
- **THEN** no rows were inserted or updated by the search request
