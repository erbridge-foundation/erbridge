## ADDED Requirements

### Requirement: Maps management surface

The system SHALL provide an account-facing `/maps` route that replaces the placeholder and lists the maps the account can see (from `GET /api/v1/maps`), each showing its name, slug, and — only when present — a summary of the ACLs the viewer can see attached to it. An empty attached-ACL list SHALL render no text (an empty list does not imply "no ACLs", since the viewer may simply lack manage permission on them). The route SHALL let the account create a map (name, slug, optional description) via a **dialog opened from a "create map" button** (not an always-present inline form), with an option to **create a default ACL** for the map at creation, and SHALL let the account soft-delete a map, each via a form action that surfaces a backend error as a handled failure (`{ action, code, message }`). Editing a map's name/slug/description is provided on the map's settings route (see below), reached from the list's edit control. Map drawing/canvas SHALL NOT be part of this surface.

When the account creates a map with the **default-ACL** option, the system SHALL create a reusable ACL named after the map, attach it to the new map, and — when the account has a main character — add that main character as an explicit `admin` member of the ACL; when the account has no main character the ACL SHALL be created and attached with no members (the owner retains implicit admin via the backend resolver). Seeding the owner member is best-effort: a failure to add the member SHALL NOT abort map creation.

#### Scenario: Create a map via the dialog

- **WHEN** the account clicks the create-map button and submits the dialog with a name and slug
- **THEN** `POST /api/v1/maps` is called and, on success, the dialog closes and the new map appears in the list

#### Scenario: Create a map with a default ACL

- **WHEN** the account submits the create dialog with the default-ACL option enabled
- **THEN** an ACL named after the map is created and attached to the map, and the account's main character (if any) is added to that ACL as an `admin` member

#### Scenario: Account sees its maps

- **WHEN** an authenticated account loads `/maps`
- **THEN** the maps from `GET /api/v1/maps` are listed, each with its name, slug, and attached-ACL summary

#### Scenario: Slug conflict surfaces a handled error

- **WHEN** the account creates a map whose slug is already taken and the backend returns a slug-conflict error
- **THEN** the create dialog shows the error message and no client-side crash occurs

#### Scenario: Soft-delete a map

- **WHEN** the account triggers delete on a map it may delete
- **THEN** `DELETE /api/v1/maps/{id}` is called and, on success, the map is removed from the list

### Requirement: Slug-keyed map canvas and settings with ACL attach/detach

The system SHALL provide a slug-keyed `/maps/[slug]` route whose `load` resolves the slug by matching it against the account's `GET /api/v1/maps` list (no backend slug-lookup endpoint), returning a 404 when no visible map has that slug. This route SHALL present the **map canvas** (a placeholder until the canvas is built) filling the whole area with no surrounding chrome bar; the map name in the list SHALL link here, and the current map's name is conveyed via the browser tab title rather than an in-canvas header. Map editing and ACL management SHALL live on a `/maps/[slug]/settings` route reached from the list's edit control, whose `load` resolves the slug the same way and 404s when absent. The settings route SHALL let the account edit the map's name/slug/description (description as a multi-line field) via a form action, attach an ACL chosen from its manageable ACLs (`POST /api/v1/maps/{id}/acls`), and detach an attached ACL (`DELETE /api/v1/maps/{id}/acls/{acl_id}`), each surfacing a backend error as a handled failure.

#### Scenario: Map name opens the canvas

- **WHEN** the account clicks a map's name in the list
- **THEN** the `/maps/{slug}` canvas route loads for that map

#### Scenario: Edit control opens settings

- **WHEN** the account activates the edit control for a map in the list
- **THEN** the `/maps/{slug}/settings` route loads with the map's editable details and its attached ACLs

#### Scenario: Unknown slug is a 404

- **WHEN** the account navigates to `/maps/{slug}` (or `/maps/{slug}/settings`) where no visible map has that slug
- **THEN** the route returns a not-found response

#### Scenario: Edit a map

- **WHEN** the account submits the settings edit form with a changed name/slug/description
- **THEN** `PATCH /api/v1/maps/{id}` is called and, on success, the change is reflected

#### Scenario: Attach an ACL

- **WHEN** the account attaches one of its manageable ACLs to the map from settings
- **THEN** `POST /api/v1/maps/{id}/acls` is called and, on success, the ACL appears in the map's attached list

#### Scenario: Detach an ACL

- **WHEN** the account detaches an attached ACL from settings
- **THEN** `DELETE /api/v1/maps/{id}/acls/{acl_id}` is called and, on success, the ACL is removed from the attached list

### Requirement: ACLs management surface

The system SHALL provide an account-facing `/acls` route listing the ACLs the account can manage (`GET /api/v1/acls`), and SHALL let the account create an ACL (name) via a **dialog opened from a "create ACL" button** (matching the maps create dialog), rename an ACL, and delete an ACL, each via a form action that surfaces backend errors as handled failures. The surface SHALL NOT display an "unattached ACL" indicator.

#### Scenario: Account sees manageable ACLs

- **WHEN** an authenticated account loads `/acls`
- **THEN** the ACLs from `GET /api/v1/acls` are listed by name

#### Scenario: Create an ACL

- **WHEN** the account opens the create dialog and submits a non-empty name
- **THEN** `POST /api/v1/acls` is called and, on success, the dialog closes and the new ACL appears in the list

#### Scenario: Rename an ACL

- **WHEN** the account renames an ACL
- **THEN** `PATCH /api/v1/acls/{id}` is called and, on success, the list reflects the new name

#### Scenario: Delete an ACL

- **WHEN** the account deletes an ACL
- **THEN** `DELETE /api/v1/acls/{id}` is called and, on success, the ACL is removed from the list

### Requirement: UUID-keyed ACL detail with member management

The system SHALL provide an `/acls/[id]` detail route (keyed by the ACL UUID) that lists the ACL's members (`GET /api/v1/acls/{id}/members`) and SHALL let the account add a member, change a member's permission, and remove a member, each via a form action calling the corresponding endpoint. A member SHALL be one of `character`, `corporation`, or `alliance`, carrying the identifier that type requires (character → `character_id` UUID; corporation/alliance → `eve_entity_id`).

#### Scenario: Detail lists members

- **WHEN** the account loads `/acls/{id}` for an ACL it can manage
- **THEN** the members from `GET /api/v1/acls/{id}/members` are listed with their type, name, and permission

#### Scenario: Add a member

- **WHEN** the account adds a member with a resolved identifier and a permission
- **THEN** `POST /api/v1/acls/{id}/members` is called and, on success, the member appears in the list

#### Scenario: Update a member's permission

- **WHEN** the account changes a member's permission
- **THEN** `PATCH /api/v1/acls/{id}/members/{member_id}` is called and, on success, the new permission is shown

#### Scenario: Remove a member

- **WHEN** the account removes a member
- **THEN** `DELETE /api/v1/acls/{id}/members/{member_id}` is called and, on success, the member is removed from the list

### Requirement: Entity-search member picker

The system SHALL provide a member picker that resolves a typed name to a member identifier via `GET /api/v1/entities/search`. The picker SHALL require at least 3 characters before searching and SHALL allow the search to be initiated by pressing Enter in the input. While a search is in flight the picker SHALL show an active-search indicator. Results SHALL be grouped by category (character/corporation/alliance); **each result row SHALL carry its own permission `<select>` and an inline "add" button** so the account adds a member in one place without a separate select-then-choose-role step. Adding a member SHALL submit the **already-resolved** identifier to the add-member action (character → `character_id` UUID; corporation/alliance → `eve_entity_id`) so no second lookup is needed. Each result SHALL show a small portrait/logo (derived from the EVE public image CDN by entity id) to aid identification when names collide. The picker SHALL surface the search "unavailable" outcome distinctly from "the search ran and matched nothing".

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

### Requirement: Client-side member-type permission gating

The system SHALL offer the `manage` and `admin` permissions only when the member being added or edited is a `character` member; for `corporation` and `alliance` members only `read`, `read_write`, and `deny` SHALL be selectable. This is a UX guard; the backend remains the authority and a rejected mutation SHALL surface as a handled failure.

#### Scenario: Manage/admin offered only for characters

- **WHEN** the account is adding a `character` member
- **THEN** the permission options include `manage` and `admin`

#### Scenario: Manage/admin withheld for corporation and alliance

- **WHEN** the account is adding a `corporation` or `alliance` member
- **THEN** the permission options exclude `manage` and `admin`

#### Scenario: Backend rejection is handled

- **WHEN** a member mutation is rejected by the backend (e.g. a CHECK violation)
- **THEN** the form shows the backend error message rather than crashing
