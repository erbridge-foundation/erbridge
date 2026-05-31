## MODIFIED Requirements

### Requirement: GET /api/v1/admin/characters/search resolves a name fragment to accounts

`GET /api/v1/admin/characters/search?q=<fragment>` SHALL return characters whose name matches the fragment (case-insensitive substring), each with its `eve_character_id`, `name`, `is_main`, owning `account_id`, `portrait_url` (a deterministic image URL derived from the character id), and `already_blocked` (whether the character is currently in the block list), so the admin UI can both resolve "promote the account that owns *Pilot X*" and reuse the same result shape in the block-character picker. The query SHALL bind `q` as a parameter (no SQL injection surface) and SHALL cap the number of returned rows.

#### Scenario: Search returns matching characters with their account
- **WHEN** a server admin calls `GET /api/v1/admin/characters/search?q=pil`
- **THEN** the response is `200` with `data` listing characters whose name contains "pil" (case-insensitive), each carrying its owning `account_id`, `portrait_url`, and `already_blocked`

#### Scenario: Search result cap
- **WHEN** a search matches more characters than the cap
- **THEN** the response returns at most the cap and does not error

#### Scenario: Result marks already-blocked characters
- **GIVEN** a character in the local table whose `eve_character_id` is in the block list
- **WHEN** it appears in a search result
- **THEN** its `already_blocked` is `true`

### Requirement: Admin frontend is gated and surfaced only to admins

The frontend SHALL provide an `/admin` route group. Its server-side load SHALL respond with HTTP 404 for any caller that is not a server admin (the existence of admin pages is not disclosed). The route group SHALL include an overview (`/admin`), admin management (`/admin/admins`), block management (`/admin/blocks`), and the audit browser (`/admin/audit`). The global navigation SHALL surface an "Admin" affordance only when the authenticated account's `is_server_admin` (from `GET /api/v1/me`) is `true`. The frontend SHALL provide a `/blocked` information page shown to a blocked pilot whose request is rejected with `account_blocked`.

The block-management page (`/admin/blocks`) SHALL block a character chosen by **name search**, not by raw EVE character ID. It SHALL search the local character index first (`GET /api/v1/admin/characters/search`); if the wanted pilot is not found locally, the admin SHALL be able to opt in to an ESI search (`GET /api/v1/admin/characters/esi-search`). Both searches require at least 3 characters. Selecting a result SHALL present a confirmation enriched with the character's corporation (fetched on selection) before the block is submitted with the resolved `eve_character_id` and an optional reason. A raw character-ID entry field SHALL NOT be present. When ESI search is unavailable, the page SHALL show a clear notice and remain usable for local-DB results.

#### Scenario: Non-admin cannot reach /admin
- **WHEN** a non-admin (or unauthenticated) user navigates to any `/admin` route
- **THEN** the server-side load returns HTTP 404; the page's existence is not disclosed

#### Scenario: Admin link shown only to admins
- **WHEN** the global navigation renders for a non-admin account
- **THEN** no "Admin" affordance is present
- **WHEN** it renders for a server-admin account
- **THEN** an "Admin" affordance linking to `/admin` is present

#### Scenario: Admin promotes by character search
- **WHEN** an admin uses `/admin/admins` to search for a character by name and confirms promotion of the owning account
- **THEN** the frontend resolves the character to its `account_id` and submits grant-admin for that account

#### Scenario: Admin blocks a character found in the local index
- **WHEN** an admin types a name fragment (≥ 3 chars) on `/admin/blocks` and the pilot appears in the local search results
- **THEN** selecting it shows a confirmation including the character's corporation, and confirming submits a block for that `eve_character_id`

#### Scenario: Admin blocks a never-seen character via ESI fallback
- **GIVEN** a griefer who has never signed in (not in the local index)
- **WHEN** the admin's local search returns nothing and the admin opts in to the ESI search
- **THEN** the ESI results appear, and selecting one blocks that `eve_character_id` (pre-emptive block)

#### Scenario: No raw character-ID entry
- **WHEN** the `/admin/blocks` page renders
- **THEN** there is no input that blocks a character by typing a raw EVE character ID

## ADDED Requirements

### Requirement: GET /api/v1/admin/characters/esi-search resolves a name fragment via ESI

`GET /api/v1/admin/characters/esi-search?q=<fragment>` SHALL search EVE characters by name against ESI, on behalf of the requesting admin's own main character, so that pilots not present in the local index (e.g. never-seen griefers) can be found for pre-emptive blocking. It SHALL be gated by `AdminAccount` like every `/api/v1/admin/*` route (cookie-only; 401 unauthenticated, 403 non-admin, 401 for a bearer key).

The endpoint SHALL require `q` to be at least 3 characters, rejecting a shorter fragment with HTTP 400. On success it SHALL return characters matching the fragment (case-insensitive substring per ESI `strict=false`), each with `eve_character_id`, `name`, `portrait_url`, and `already_blocked`, capped at a sensible maximum.

When the search cannot be performed — the admin's main character has no usable token, the token cannot be refreshed, the token lacks the `esi-search.search_structures.v1` scope, or ESI is unavailable — the endpoint SHALL respond `200` with an empty result list and a machine-readable `unavailable` indicator (e.g. `esi_search_unavailable`) rather than a 5xx, so the UI can show "ESI search unavailable — re-authorise your character" without breaking the block flow. The admin's access token SHALL NOT appear in the response.

#### Scenario: ESI search returns matching characters
- **WHEN** an admin with a usable, scoped token calls `GET /api/v1/admin/characters/esi-search?q=wasp` and ESI matches "Wasp 223"
- **THEN** the response is `200` with `data` listing the matched characters, each carrying `eve_character_id`, `name`, `portrait_url`, and `already_blocked`, and the `unavailable` indicator is false/absent

#### Scenario: Fragment shorter than 3 characters is rejected
- **WHEN** an admin calls `GET /api/v1/admin/characters/esi-search?q=wa`
- **THEN** the response is HTTP 400 and no ESI request is made

#### Scenario: ESI search unavailable degrades gracefully
- **WHEN** an admin's token lacks the search scope (or is unrefreshable, or ESI is unreachable)
- **THEN** the response is `200` with an empty `data` list and the `unavailable` indicator set; it is not a 5xx, and the admin's token is never disclosed

#### Scenario: Admin gating
- **WHEN** the endpoint is called with no session, a non-admin session, or a bearer API key
- **THEN** the response is 401 (no session / bearer), or 403 (non-admin session) — identical to every other admin route
