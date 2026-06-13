# Optimize Entity Search

## Why

A backend review (2026-06-11) found the entity search has two costly behaviours: (1) every matched id is name-resolved with its own sequential ESI public-info GET — a single member-picker search can issue up to ~75 outbound ESI calls (25 per category), painful for latency and for the ESI error budget, especially with rate limiting not yet deployed; (2) every character match with no local row mints a permanent orphan `eve_character` row *at search time* — any authenticated account can grow the table by 25 placeholder rows per search keystroke, including rows with `(0, "")` corporation placeholders when public-info fails.

## What Changes

- Replace per-id name resolution with ESI's bulk `POST /universe/names/` endpoint: one outbound call resolves all matched ids across categories. Unresolvable ids are still dropped.
- Stop minting orphans during search. Search results for characters carry the `eve_character_id`, `name`, and — when a local row already exists — its `id` UUID; otherwise the UUID is absent.
- Mint at member-add instead. The add-member request already carries `eve_entity_id` (the durable EVE id, mandatory for every member type since `make-audit-log-self-contained`) and `character_id` (the `eve_character.id` UUID). This change makes `character_id` *optional* for character members: when present the existing row is referenced as today; when absent the service find-or-mints the orphan from `eve_entity_id` inside the add, then inserts the member. No new request field is introduced. Minting moves from "every search result" to "the one entity actually selected".
- Batch the admin search's per-row blocked-status checks (`is_eve_character_blocked` N+1) into a single `ANY($ids)` query.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `esi-search`: name resolution becomes a single bulk `POST /universe/names/` call instead of per-id public-info GETs.
- `entity-search`: character results no longer mint orphans; the UUID is present only for already-known characters.
- `acls`: a character member may be added with `eve_entity_id` and no `character_id`, minting the orphan at add time; `character_id` becomes optional for character members.
- `data-persistence`: the "orphan characters may be minted from entity search" requirement moves its mint point to the ACL member add.

## Impact

- Backend: `esi/search.rs` (bulk resolver), `services/entity_search.rs` (no mint path; UUID lookup batch), `services/acl.rs` (`validate_member_shape` makes `character_id` optional; `add_member` gains the mint-when-absent branch — `dto/acl.rs` needs no new field), `services/admin.rs` (batched blocked check, `db/blocks.rs` gains the `ANY` query).
- Frontend: `MemberPicker.svelte` submits `character_id` only when the search result carries a UUID, always submits `eve_entity_id`; `acls/[id]` add-member action forwards `character_id` only when present.
- API contract: `EntitySearchPageDto.characters[].id` becomes nullable; `AddMemberRequest.character_id` becomes optional for character members (the field already exists — no new field). Both are extensions the existing frontend tolerates only after its own update — ships together.
- Tests: wiremock bulk-names endpoint; integration tests for mint-on-add; HURL updates for both endpoints.
