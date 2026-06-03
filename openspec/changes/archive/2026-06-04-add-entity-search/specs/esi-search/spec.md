## MODIFIED Requirements

### Requirement: Authenticated ESI character-name search on behalf of a character

The system SHALL provide a function that searches EVE entities by name fragment against ESI, authenticated as a specific character, across one or more of the `character`, `corporation`, and `alliance` categories. It SHALL call `GET /characters/{character_id}/search/` with query parameters `categories=<comma-separated categories>`, `search=<fragment>`, and `strict=false` (case-insensitive substring match), sending the required `X-Compatibility-Date` header and an `Authorization: Bearer <access_token>` header carrying that character's access token. The required ESI scope is `esi-search.search_structures.v1` (already requested by the SSO flow).

ESI requires the `search` fragment to be at least 3 characters; the function SHALL NOT issue a request for a shorter fragment.

The ESI response is an object whose keys are the requested categories, each mapping to an array of EVE ids. The function SHALL resolve each returned id to its name via public-info — characters via the character public-info path, corporations via the corporation public-info path, alliances via the alliance public-info path — yielding `(eve_id, name)` pairs partitioned by category. An id whose name cannot be resolved SHALL be dropped so results are always displayable. The number of resolved results SHALL be capped at a sensible maximum per category.

This is the first authenticated outbound ESI call in the backend. The function SHALL accept the caller's already-decrypted access token; token storage, decryption, and refresh are the caller's responsibility (see the token-availability requirement).

#### Scenario: Character search returns matching characters resolved to names

- **WHEN** the function is called for the `character` category with a valid character access token (with the search scope) and a fragment of at least 3 characters that ESI matches
- **THEN** it returns the matched characters as `(eve_character_id, name)` pairs (each id resolved to its current name), capped at the maximum

#### Scenario: Corporation and alliance categories resolve to names

- **WHEN** the function is called for the `corporation` and `alliance` categories with a matching fragment
- **THEN** it returns the matched corporations and alliances as `(eve_entity_id, name)` pairs partitioned by category, each id resolved to its current name via public-info

#### Scenario: Multiple categories in one call

- **WHEN** the function is called with `categories` of `character,corporation,alliance` and a matching fragment
- **THEN** a single ESI search request is issued and the matches are returned grouped by category

#### Scenario: Fragment shorter than 3 characters is not sent to ESI

- **WHEN** the function is called with a fragment shorter than 3 characters
- **THEN** no ESI request is made and the function reports the too-short condition to the caller (it does not return matches)

#### Scenario: Substring, not exact, match

- **WHEN** the function searches with fragment "wasp" and ESI holds a character named "Wasp 223"
- **THEN** "Wasp 223" is among the resolved results (`strict=false` yields a substring match)

#### Scenario: Unresolvable id is dropped

- **WHEN** ESI returns an id whose public-info name lookup fails
- **THEN** that id is omitted from the results rather than appearing without a name
