## Purpose

Authenticated EVE Swagger Interface (ESI) character-name search performed on behalf of a specific character, using that character's stored EVE access token (best-effort refreshed). Provides the backend function that resolves a name fragment to `(eve_character_id, name)` pairs via ESI's character-search endpoint, distinguishes "no matches" from "search unavailable", and never discloses the access token to any client. This is the first authenticated outbound ESI call in the backend.

## Requirements

### Requirement: Authenticated ESI character-name search on behalf of a character

The system SHALL provide a function that searches EVE characters by name fragment against ESI, authenticated as a specific character. It SHALL call `GET /characters/{character_id}/search/` with query parameters `categories=character`, `search=<fragment>`, and `strict=false` (case-insensitive substring match), sending the required `X-Compatibility-Date` header and an `Authorization: Bearer <access_token>` header carrying that character's access token. The required ESI scope is `esi-search.search_structures.v1` (already requested by the SSO flow).

ESI requires the `search` fragment to be at least 3 characters; the function SHALL NOT issue a request for a shorter fragment.

The ESI response for the `character` category is an array of character IDs. The function SHALL resolve each returned ID to its name (via public-info), yielding `(eve_character_id, name)` pairs. The number of resolved results SHALL be capped at a sensible maximum.

This is the first authenticated outbound ESI call in the backend. The function SHALL accept the caller's already-decrypted access token; token storage, decryption, and refresh are the caller's responsibility (see the token-availability requirement).

#### Scenario: Search returns matching characters resolved to names
- **WHEN** the function is called with a valid character access token (with the search scope) and a fragment of at least 3 characters that ESI matches
- **THEN** it returns the matched characters as `(eve_character_id, name)` pairs (each id resolved to its current name), capped at the maximum

#### Scenario: Fragment shorter than 3 characters is not sent to ESI
- **WHEN** the function is called with a fragment shorter than 3 characters
- **THEN** no ESI request is made and the function reports the too-short condition to the caller (it does not return matches)

#### Scenario: Substring, not exact, match
- **WHEN** the function searches with fragment "wasp" and ESI holds a character named "Wasp 223"
- **THEN** "Wasp 223" is among the resolved results (`strict=false` yields a substring match)

### Requirement: ESI search degrades gracefully when the token or ESI is unavailable

The ESI search SHALL distinguish "ESI returned no matches" from "the search could not be performed". The search could-not-be-performed cases are: the character has no usable access token (cleared, never present), the token is expired and cannot be refreshed, the token lacks the `esi-search.search_structures.v1` scope (ESI 403), or ESI is unreachable / returns a non-success status. In every such case the function SHALL return a distinct "unavailable" outcome (not an error that propagates as a 5xx, and not an empty-success that looks like "no matches"), carrying a machine-readable reason the caller can surface.

A search that completes but matches nothing SHALL return an empty-but-available result, distinct from the unavailable outcome.

#### Scenario: Missing scope yields an unavailable outcome, not a crash
- **WHEN** the character's token is valid but lacks the search scope and ESI responds 403
- **THEN** the function returns the "unavailable" outcome with a reason indicating the search could not be performed; it does not error out and does not return an empty match list as if nothing matched

#### Scenario: ESI unreachable yields an unavailable outcome
- **WHEN** ESI is unreachable or returns a non-success status for the search
- **THEN** the function returns the "unavailable" outcome with a reason; the caller can still operate on other data sources

#### Scenario: A completed search with no matches is distinct from unavailable
- **WHEN** the search completes successfully but ESI returns no character IDs
- **THEN** the function returns an empty, available result — distinguishable from the unavailable outcome

### Requirement: ESI search uses the requesting character's token, best-effort refreshed

When a search is performed on behalf of a character, the system SHALL use that character's stored EVE access token. If the stored access token is expired, the system SHALL attempt a best-effort refresh using the stored refresh token before the search. If no usable token can be obtained (no refresh token, refresh fails), the search SHALL resolve to the "unavailable" outcome rather than erroring.

The access token SHALL be decrypted only transiently for the outbound ESI call and SHALL NOT be returned to any client.

#### Scenario: Expired token is refreshed before searching
- **GIVEN** a character whose stored access token is expired but whose refresh token is valid
- **WHEN** a search is performed on its behalf
- **THEN** the token is refreshed first and the search proceeds with the refreshed token

#### Scenario: Unrefreshable token resolves to unavailable
- **GIVEN** a character whose access token is expired and whose refresh token is absent or rejected
- **WHEN** a search is performed on its behalf
- **THEN** the search resolves to the "unavailable" outcome; no client ever receives the token
