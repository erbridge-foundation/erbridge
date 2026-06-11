# Optimize Entity Search

## Why

A backend review (2026-06-11) found the entity search has two costly behaviours: (1) every matched id is name-resolved with its own sequential ESI public-info GET — a single member-picker search can issue up to ~75 outbound ESI calls (25 per category), painful for latency and for the ESI error budget, especially with rate limiting not yet deployed; (2) every character match with no local row mints a permanent orphan `eve_character` row *at search time* — any authenticated account can grow the table by 25 placeholder rows per search keystroke, including rows with `(0, "")` corporation placeholders when public-info fails.

## What Changes

- Replace per-id name resolution with ESI's bulk `POST /universe/names/` endpoint: one outbound call resolves all matched ids across categories. Unresolvable ids are still dropped.
- Stop minting orphans during search. Search results for characters carry the `eve_character_id`, `name`, and — when a local row already exists — its `id` UUID; otherwise the UUID is absent.
- Mint at member-add instead: `POST /api/v1/acls/{acl_id}/members` accepts a character member identified by *either* `character_id` (existing row) or `eve_character_id` (no row yet — the service mints the orphan inside the add, then inserts the member). Minting moves from "every search result" to "the one entity actually selected".
- Batch the admin search's per-row blocked-status checks (`is_eve_character_blocked` N+1) into a single `ANY($ids)` query.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `esi-search`: name resolution becomes a single bulk `POST /universe/names/` call instead of per-id public-info GETs.
- `entity-search`: character results no longer mint orphans; the UUID is present only for already-known characters.
- `acls`: character members may be added by `eve_character_id`, minting the orphan at add time.
- `data-persistence`: the "orphan characters may be minted from entity search" requirement moves its mint point to the ACL member add.

## Impact

- Backend: `esi/search.rs` (bulk resolver), `services/entity_search.rs` (no mint path; UUID lookup batch), `services/acl.rs` + `dto/acl.rs` (add-member accepts `eve_character_id`), `services/admin.rs` (batched blocked check, `db/blocks.rs` gains the `ANY` query).
- Frontend: `MemberPicker.svelte` submits `eve_character_id` when no UUID is present; `acls/[id]` add-member action forwards it.
- API contract: `EntitySearchPageDto.characters[].id` becomes nullable; `AddMemberRequest` gains `eve_character_id` for character members. Both are extensions the existing frontend tolerates only after its own update — ships together.
- Tests: wiremock bulk-names endpoint; integration tests for mint-on-add; HURL updates for both endpoints.
