## Purpose

An account-authenticated HTTP search over EVE characters, corporations, and alliances by name fragment, performed on behalf of one of the requesting account's characters. It resolves a typed name to the identifier ACL membership needs — a character to its `eve_character.id` UUID (minting a ghost/orphaned `eve_character` row when the character has no row yet), a corporation or alliance to its numeric `eve_entity_id` — and degrades gracefully to an "unavailable" outcome when the search cannot be performed. The single endpoint the maps/ACLs member picker builds on; the admin character search consumes the same shared path.

## Requirements

### Requirement: Account-authenticated entity search endpoint

The system SHALL provide an account-authenticated HTTP endpoint `GET /api/v1/entities/search` that searches EVE entities by name fragment on behalf of the requesting account. The endpoint SHALL accept a `q` query parameter (the name fragment) and an optional `categories` query parameter (a comma-separated subset of `character`, `corporation`, `alliance`); when `categories` is omitted the endpoint SHALL search all three.

The endpoint SHALL require an authenticated account (session cookie or bearer token) and SHALL NOT require server-admin privileges. The search SHALL be performed on behalf of one of the requesting account's characters, using that character's EVE access token (best-effort refreshed), and SHALL NOT disclose that token to any client.

The fragment SHALL be at least 3 characters; a shorter fragment SHALL be rejected with a client error before any ESI request is made.

The response SHALL group matched entities by category, and each result SHALL carry the identifier its member type requires for ACL membership:

- a `character` result SHALL carry the `eve_character.id` UUID and the character name;
- a `corporation` or `alliance` result SHALL carry the numeric `eve_entity_id` and the entity name.

#### Scenario: Authenticated account searches all categories

- **WHEN** an authenticated account requests `GET /api/v1/entities/search?q=wasp` with a usable character token
- **THEN** the response returns matched characters, corporations, and alliances grouped by category, each carrying its identifier and name

#### Scenario: Categories filter restricts the search

- **WHEN** an authenticated account requests `GET /api/v1/entities/search?q=wasp&categories=corporation`
- **THEN** only corporation matches are returned and no character or alliance results are included

#### Scenario: Fragment shorter than 3 characters is rejected

- **WHEN** an authenticated account requests the endpoint with a `q` shorter than 3 characters
- **THEN** the request is rejected with a client error and no ESI request is made

#### Scenario: Unauthenticated request is refused

- **WHEN** an unauthenticated request hits the endpoint
- **THEN** the request is refused with an unauthorized error

#### Scenario: Non-admin account may search

- **WHEN** an authenticated non-admin account uses the endpoint
- **THEN** the search proceeds (the endpoint does not require server-admin privileges)

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

### Requirement: Entity search degrades gracefully when search cannot be performed

The endpoint SHALL distinguish "the search ran and matched nothing" from "the search could not be performed". When the requesting account has no character with a usable (best-effort-refreshed) token, when the token lacks the required search scope, or when ESI is unreachable or rejects the request, the endpoint SHALL return a graceful "unavailable" outcome rather than a 5xx and rather than an empty result that looks like "no matches". A search that completes but matches nothing SHALL return an empty-but-available result.

#### Scenario: No usable token yields unavailable, not an error

- **WHEN** the requesting account has no character with a usable token
- **THEN** the endpoint returns the "unavailable" outcome (not a 5xx, not an empty match list presented as "no matches")

#### Scenario: Completed search with no matches is available and empty

- **WHEN** the search completes successfully but ESI returns no matches in any requested category
- **THEN** the endpoint returns an empty, available result, distinguishable from the unavailable outcome

### Requirement: Admin character search delegates to the shared entity search

The existing admin character-search endpoint (`/api/v1/admin/characters/esi-search`) SHALL consume the shared entity-search path (character category only) rather than maintaining its own ESI-search logic. Its existing request and response contract and its server-admin authorization SHALL be preserved.

#### Scenario: Admin character search still returns character matches

- **WHEN** a server admin uses the admin character-search endpoint with a fragment ESI matches
- **THEN** the matched characters are returned with the endpoint's existing response shape, via the shared search path

#### Scenario: Admin search remains admin-only

- **WHEN** a non-admin account calls the admin character-search endpoint
- **THEN** the request is refused, unchanged from prior behavior
