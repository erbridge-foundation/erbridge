## Why

Adding a member to an ACL requires resolving a real EVE entity — a character, corporation, or alliance — to the identifier the `acl_member` row stores. Today the only name search is **admin-only**, covers **characters only**, and returns the numeric `eve_character_id` rather than the `eve_character.id` UUID an `acl_member` needs. There is **no** corporation or alliance search at all. The maps + ACLs management UI cannot offer a usable member picker on this foundation, so this change builds the shared, account-authenticated entity-search backend the UI will consume.

## What Changes

- Generalize the existing authenticated ESI search function so it can search the `character`, `corporation`, and `alliance` categories (today it is hard-coded to `character`), resolving matched ids to display names.
- Introduce a **common, account-authenticated** entity-search HTTP endpoint usable by any authenticated account (not just admins). It searches characters and/or corporations and/or alliances by name fragment, on behalf of one of the requesting account's characters (best-effort token refresh), and returns displayable results carrying the identifier each member type needs:
  - **character** → the `eve_character.id` UUID (not just the numeric EVE id);
  - **corporation** / **alliance** → the `eve_entity_id` (numeric EVE id) + name.
- **Ghost characters**: when a character returned by ESI search is selected but has no `eve_character` row yet, the system mints a **ghost/orphaned character** — an `eve_character` with `account_id = NULL` and no tokens — so it has a stable `eve_character.id` UUID that an `acl_member` can reference and the map permission resolver can match. (`eve_character.account_id` is already nullable; no migration required.)
- Refactor the existing admin character-search endpoint (`/api/v1/admin/characters/esi-search`) to consume the shared search path rather than carry its own copy.
- HURL coverage for the new endpoint alongside the existing `cargo test` (unit + integration) suite.

This change is **backend-only**. It is the dependency for the follow-up frontend change `add-maps-and-acls-ui`, which builds the maps/ACLs management surface and the member picker on top of it.

## Capabilities

### New Capabilities

- `entity-search`: An account-authenticated HTTP search over EVE characters, corporations, and alliances by name fragment, performed on behalf of one of the account's characters. Returns results carrying the per-type identifier ACL membership needs (character → `eve_character.id` UUID, corp/alliance → `eve_entity_id`), minting a ghost/orphaned `eve_character` row for a searched character that has no row yet. The single new endpoint other accounts and the maps/ACLs UI build on.

### Modified Capabilities

- `esi-search`: The authenticated ESI search function is generalized from the `character` category only to any of `character`, `corporation`, `alliance` (one or more categories per call), with name resolution for corporations and alliances in addition to characters. The "no matches" vs "unavailable" outcome distinction and the never-disclose-the-token guarantee are unchanged.
- `data-persistence`: Defines the **ghost/orphaned character** as a durable lifecycle state of `eve_character` — a row with `account_id = NULL`, no ESI tokens, and `is_main = false`, populated from ESI public info. Establishes that such a row is a valid, referenceable identity (e.g. as an `acl_member` target) despite belonging to no account.

## Impact

- **Backend code**:
  - `esi/search.rs` — generalize `character_search` to accept categories; add corporation/alliance name resolution (reusing `esi/public_info.rs`'s `fetch_corporation_name` / `fetch_alliance_name`).
  - `services/` — new entity-search service orchestrating token acquisition (reusing the admin path's usable-token logic), the multi-category ESI search, character→UUID resolution, and ghost-character minting; `services/admin.rs` refactored to delegate.
  - `db/characters.rs` — a "find-or-mint ghost by `eve_character_id`" path (reusing the existing insert).
  - `handlers/api/v1/` — a new account-authenticated search handler + route; `handlers/api/v1/admin.rs` refactored to call the shared service.
  - `dto/` + `response.rs` — request/response DTOs for the unified search result.
- **APIs**: one new `GET /api/v1/…` search endpoint (exact path in design.md); admin ESI character-search behavior preserved.
- **Schema**: no migration — relies on the existing nullable `eve_character.account_id`.
- **Tests**: `cargo test` unit + integration coverage; new HURL suite for the endpoint.
- **Dependencies**: none new expected (reuses existing `reqwest`/`wiremock` test stack and ESI plumbing).
