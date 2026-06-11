## MODIFIED Requirements

### Requirement: Entity-search member picker

The system SHALL provide a member picker that resolves a typed name to a member identifier via `GET /api/v1/entities/search`. The picker SHALL require at least 3 characters before searching and SHALL allow the search to be initiated by pressing Enter in the input. While a search is in flight the picker SHALL show an active-search indicator. The picker SHALL offer a **search scope** control with the options `character`, `corporation`, `alliance`, and `any`, defaulting to `any`; the chosen scope SHALL narrow the search to that single ESI category, while `any` SHALL search all categories. The scope SHALL be submitted with the search request so the `categories` parameter of `GET /api/v1/entities/search` is set to the chosen category, or omitted (backend all-categories default) when the scope is `any`. Results SHALL be grouped by category (character/corporation/alliance); **each result row SHALL carry its own permission `<select>` and an inline "add" button** so the account adds a member in one place without a separate select-then-choose-role step. Adding a member SHALL submit the **already-resolved** identifier to the add-member action (character → `character_id` UUID; corporation/alliance → `eve_entity_id`) so no second lookup is needed. Each result SHALL show a small portrait/logo (derived from the EVE public image CDN by entity id) to aid identification when names collide. The picker SHALL surface the search "unavailable" outcome distinctly from "the search ran and matched nothing".

#### Scenario: Add a character from a result row

- **WHEN** the account searches, chooses a permission in a returned character's row, and clicks its add button
- **THEN** the add-member action receives that character's `eve_character.id` UUID as `character_id` with the chosen permission

#### Scenario: Add a corporation or alliance from a result row

- **WHEN** the account adds a returned corporation or alliance from its row
- **THEN** the add-member action receives that entity's numeric `eve_entity_id` with the chosen permission

#### Scenario: Search can be initiated with Enter

- **WHEN** the account has typed at least 3 characters and presses Enter in the search input
- **THEN** the search runs without requiring a click on the search button

#### Scenario: Active search shows an indicator

- **WHEN** a search request is in flight
- **THEN** the picker shows a visible "searching" indicator until results return

#### Scenario: Fragment shorter than 3 characters is not searched

- **WHEN** the account types fewer than 3 characters
- **THEN** no search request is made and the picker prompts for more characters

#### Scenario: Results show a portrait

- **WHEN** the picker renders grouped results
- **THEN** each result displays a small portrait/logo for that character/corporation/alliance

#### Scenario: Search unavailable is distinct from no matches

- **WHEN** the entity search returns the "unavailable" outcome
- **THEN** the picker shows a "search unavailable" state, distinct from an empty "no matches" result

#### Scenario: Scope narrows the search to one category

- **WHEN** the account selects a scope of `corporation` (or `character`, or `alliance`) and runs a search
- **THEN** `GET /api/v1/entities/search` is called with `categories` set to that single category and only that category's results are returned

#### Scenario: Scope "any" searches all categories

- **WHEN** the account leaves the scope at its `any` default and runs a search
- **THEN** the search request omits the `categories` parameter and the backend searches all three categories
