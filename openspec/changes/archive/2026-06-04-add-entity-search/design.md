## Context

The maps + ACLs backend (archived `add-maps-and-acls`) lets an account add `acl_member` rows that grant a permission to a **character** (by `eve_character.id` UUID), a **corporation**, or an **alliance** (both by numeric `eve_entity_id`). The map permission resolver matches an account's characters against those members. But there is currently no way for the UI to turn a name the user types into one of those identifiers:

- `esi/search.rs::character_search` hard-codes `categories=character` and returns numeric `eve_character_id`s; `resolve_character_names` turns them into `(id, name)` pairs.
- `services/admin.rs::esi_search_characters` wraps that for the admin UI only, behind `/api/v1/admin/characters/esi-search`, and owns the "find one of the account's characters with a usable (best-effort-refreshed) token" logic (`get_usable_main_access_token`).
- `esi/public_info.rs` already has `fetch_corporation_name` and `fetch_alliance_name`, plus a character public-info fetch (name + corp id) used for block snapshots.
- `eve_character.account_id` is **nullable**; `data-persistence` already specifies the `account_id = NULL` row as an **orphan** ("public-info cache populated by flows like map-ACL pre-claim"), with NULL tokens and empty scopes.

So most primitives exist; this change composes them into one shared, account-authenticated endpoint and adds corporation/alliance categories plus the character→UUID (mint-orphan-if-absent) step.

## Goals / Non-Goals

**Goals:**
- One account-authenticated search endpoint the maps/ACLs UI can call to populate an ACL-member picker for all three member types.
- Generalize the ESI search function to the `character`, `corporation`, and `alliance` categories in a single call, preserving the existing "no matches" vs "unavailable" outcome distinction and the never-disclose-the-token guarantee.
- Return identifiers each member type needs directly: character results carry the `eve_character.id` UUID (minting an orphan when absent); corp/alliance results carry the numeric `eve_entity_id`.
- Refactor the admin character-search endpoint to consume the shared path, removing the duplicated search logic.
- Full test coverage: unit (wiremock ESI), integration, and a HURL suite.

**Non-Goals:**
- Any frontend work (that is `add-maps-and-acls-ui`).
- A new migration — the orphan row state already exists in the schema.
- Caching, pagination, or rate-limiting of search beyond the existing result cap.
- Backfilling corp/alliance rows into a local table — corporations and alliances are referenced by numeric id only (`acl_member.eve_entity_id`); there is no `eve_entity` table and this change does not add one.

## Decisions

### Decision: One endpoint, multi-category — `GET /api/v1/entities/search`

A single account-authenticated endpoint: `GET /api/v1/entities/search?q=<fragment>&categories=character,corporation,alliance`. `categories` is optional and defaults to all three; the response groups results by type. The `q` length floor (`MIN_SEARCH_LEN = 3`, an ESI constraint) is validated in the handler exactly as the admin endpoint does today.

- **Alternatives considered**: three separate endpoints (`/entities/characters/search`, `/corporations/search`, `/alliances/search`). Rejected — ESI's `/search/` endpoint takes a comma-separated `categories` list and returns all matches in one round-trip, so one call is cheaper and the UI picker wants a blended result anyway. A `categories` filter preserves the ability to scope when the caller only wants one type.
- Path lives under a new `/api/v1/entities/` namespace rather than `/characters/` because it spans characters, corps, and alliances.

### Decision: Generalize `esi::search::character_search` rather than add a parallel function

Rename/extend the existing function to accept a set of categories and return a richer result (ids partitioned by category), then resolve each category's ids to names via the existing public-info helpers (`resolve_character_names` for characters; `fetch_corporation_name` / `fetch_alliance_name` for the other two). The `EsiSearchError { Rejected, Unavailable }` outcome type and the graceful-degradation contract are preserved unchanged.

- **Alternatives considered**: leave `character_search` alone and write a sibling `entity_search`. Rejected — it would duplicate the request-building, header, and error-mapping logic the existing function already has well-tested; the `esi-search` spec is the right home for the generalization.

### Decision: Character results resolve to the `eve_character.id` UUID, minting an orphan when absent

The ACL member needs `eve_character.id` (UUID), but ESI search yields `eve_character_id` (numeric). For each matched character the service does a find-or-mint:

1. `SELECT id FROM eve_character WHERE eve_character_id = $1` — if present, return that UUID (it may belong to an account or be an existing orphan; either is fine).
2. If absent, **mint an orphan**: fetch the character's public info (name, corp id → corp name, alliance id → alliance name) and `INSERT` an `eve_character` row with `account_id = NULL`, NULL tokens, empty scopes, `is_main = false`. Return the new UUID.

This makes every character result immediately usable as an `acl_member.character_id` and resolver-matchable, and aligns with the pre-existing "orphan = public-info cache" model in `data-persistence`.

- **Why mint at search time (not at add-member time)**: returning a stable UUID in the search result lets the picker send the UUID straight to `POST /acls/{id}/members` with no second lookup, and keeps the orphan-minting concern in one place. The trade-off (orphans created for characters the user merely searched and never added) is acceptable — an orphan is a cheap public-info cache row with no tokens, exactly what the schema intends; it is also self-healing (claimed on that pilot's next login per the existing orphan-claim flow).
- **Alternatives considered**: return only the numeric id and mint lazily inside the add-member service. Rejected — it pushes ESI public-info fetching into the ACL service and forces the picker to round-trip; minting at resolution time keeps the boundary clean. (If orphan churn ever matters, a future change can gate minting behind selection rather than search — noted as an open question.)

### Decision: Admin endpoint delegates to the shared service

`services/admin.rs::esi_search_characters` is reduced to a thin call into the new entity-search service (character category only), preserving the admin endpoint's existing request/response contract and its admin-only authorization. The admin path's `get_usable_main_access_token` logic moves into (or is shared with) the new service so both paths refresh tokens identically.

- **Alternatives considered**: leave the admin path untouched and accept duplication. Rejected — the proposal explicitly calls for the refactor, and a single token-acquisition path is less to keep correct.

### Decision: Token acquisition reuses the account's usable main character token

The search runs on behalf of the requesting account, using one of its characters' tokens (best-effort refreshed), mirroring the admin path. An account with no usable token resolves to the existing "unavailable" outcome — the endpoint returns a graceful unavailable response, never a 5xx.

## Risks / Trade-offs

- **Orphan churn from searching** → minting at search time can create orphan rows for characters never added to any ACL. Mitigation: orphans are cheap, token-less public-info rows the schema already sanctions; they self-heal on the pilot's next login. If churn becomes a problem, gate minting on selection in a later change.
- **Stale orphan public-info** → an orphan's corp/alliance snapshot is frozen at mint time. Mitigation: the existing background refresh / next-login upsert already refreshes public-info columns; no new staleness path is introduced versus the pre-existing orphan model.
- **ESI corp/alliance name resolution failures** → a matched corp/alliance id whose name lookup fails would otherwise yield a nameless result. Mitigation: follow the existing best-effort pattern — drop ids that fail to resolve (as `resolve_character_names` already does), so results are always displayable.
- **Account with no search-scoped token** → the endpoint cannot search. Mitigation: the existing "unavailable" outcome already covers this; the UI surfaces it as "search unavailable", distinct from "no matches".
- **Exposing search to all accounts** (vs admin-only today) → broader surface. Mitigation: it is read-only public-info name resolution authenticated as the caller's own character, identical in capability to data already visible in-game; no token is ever disclosed.

## Open Questions

- Should orphan minting happen at search time (current decision) or be deferred until a character is actually selected/added? Deferring reduces orphan churn at the cost of a second round-trip; revisit if churn is observed.
- Should the response cap be per-category or shared across categories? Starting with a per-category cap mirroring the existing character cap; tune if the blended picker feels unbalanced.
