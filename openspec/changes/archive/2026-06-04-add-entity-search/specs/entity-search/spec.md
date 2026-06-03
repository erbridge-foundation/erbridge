## ADDED Requirements

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

### Requirement: Character results resolve to a referenceable character UUID, minting an orphan when absent

For every character matched by the search, the endpoint SHALL return the `eve_character.id` UUID, not merely the numeric `eve_character_id`. If an `eve_character` row already exists for the matched `eve_character_id` (whether account-owned or an existing orphan), the endpoint SHALL return that row's `id`. If no row exists, the endpoint SHALL mint an **orphan** `eve_character` row — `account_id = NULL`, no ESI tokens, `is_main = false`, populated from ESI public info (name, corporation id and name, alliance id and name when present) — and return the new row's `id`.

The returned UUID SHALL be immediately usable as an `acl_member.character_id` and matchable by the map permission resolver.

#### Scenario: Existing character resolves to its UUID

- **WHEN** a matched character already has an `eve_character` row
- **THEN** the result carries that row's `id` UUID and no new row is created

#### Scenario: Unknown character is minted as an orphan

- **WHEN** a matched character has no `eve_character` row
- **THEN** a new `eve_character` row is inserted with `account_id = NULL`, NULL token columns, `is_main = false`, and public-info columns populated from ESI, and the result carries the new row's `id`

#### Scenario: Minted orphan is referenceable as an ACL member

- **WHEN** a character UUID returned by the search is used as an `acl_member.character_id`
- **THEN** the member is created successfully and the map permission resolver matches the owning account's characters against it

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
