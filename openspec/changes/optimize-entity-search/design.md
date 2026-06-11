# Design — optimize-entity-search

## Context

`esi/search.rs::resolve_named` loops over matched ids issuing one `GET {category}/{id}/` each, sequentially, capped at 25 per category. `services/entity_search.rs::find_or_mint_character` then mints an orphan row for every character match without a local row, fetching that character's affiliations (up to two more ESI calls each). The admin search annotates each result with a per-row `is_eve_character_blocked` query. The spec (`data-persistence`) currently *requires* mint-on-search; this change moves that requirement rather than violating it.

## Goals / Non-Goals

**Goals:**
- One ESI round-trip for all name resolution in a search.
- Orphan rows are created only for entities a user actually adds to an ACL.
- Admin search issues O(1) blocked-status queries.

**Non-Goals:**
- Reaping orphans already minted by past searches (they are valid identities; harmless rows stay).
- Outbound rate limiting / error-limit backoff (owned by `add-esi-rate-limit-backoff`; this change reduces pressure but doesn't gate).
- Caching name resolutions across searches.

## Decisions

**Bulk resolution via `POST /universe/names/`.** ESI resolves up to 1 000 mixed-category ids per call, returning `{ id, name, category }` objects. One call covers all three categories at once — better than three parallel per-category fans. Ids the endpoint cannot resolve cause a 404 *for the whole batch* only when **all** ids are invalid; ESI otherwise omits unknown ids — the existing "drop unresolvable ids" semantic maps cleanly. The per-id GET fallback is deleted, not kept as a fallback path (one contract, one code path; if the bulk endpoint is down, search degrades to `Unavailable`, which is already a spec-blessed outcome). The response's `category` field also lets the resolver partition results without tracking which id came from which request array.

**Search returns `id: Option<Uuid>` for characters.** `services/entity_search.rs` batch-looks-up existing rows (`WHERE eve_character_id = ANY($1)` — replacing N `find_id_by_eve_character_id` calls) and attaches the UUID where found. No write happens in the search path at all, which also makes search safely retryable/spammable.

**Mint inside the member-add transaction.** `AddMemberInput` for character members carries `character_id: Option<Uuid>` OR `eve_character_id: Option<i64>` (exactly one; shape-validated like the existing type/id coherence rules). When `eve_character_id` is given: inside the add transaction, find-or-mint the orphan (public-info affiliation fetch happens *before* the tx opens — no ESI call mid-transaction, keeping lock hold time bounded), then insert the member with the resolved UUID. The mint keeps the placeholder-on-public-info-failure behaviour: the selected entity must be addable even when ESI affiliation lookup fails; name comes from the search result the user clicked.

**Blocked-status batch.** `db/blocks.rs` gains `blocked_set(pool, &[i64]) -> HashSet<i64>` (`SELECT eve_character_id FROM blocked_eve_character WHERE eve_character_id = ANY($1)`); `services/admin.rs` annotates from the set. The single-id `is_eve_character_blocked` stays for the SSO callback's one-character check.

**Race: concurrent claim/mint.** Two adds for the same unknown character can race the mint. `eve_character.eve_character_id` is UNIQUE; the second insert hits the constraint and retries as find (`ON CONFLICT (eve_character_id) DO NOTHING` + re-select inside the tx). Same pattern if the pilot logs in concurrently — login's upsert and the mint serialise on the unique index either way.

## Risks / Trade-offs

- [Frontend/backend lockstep] `characters[].id` becomes nullable and add-member gains a field — old frontend against new backend would still work (it only ever sends `character_id`s it got from search, which new backend still accepts); new frontend against old backend would fail for unknown characters. → Ship as one change; the compose stack deploys both together.
- [Bulk endpoint quirk] `POST /universe/names/` 404s if *every* id is unknown. → Treat 404 with a non-empty request as "all dropped", not `Unavailable`.
- [Slightly stale names] Bulk resolution returns current names identically to per-id; no behaviour change.
- [Spec movement] The `data-persistence` mint requirement is MODIFIED to relocate the mint point; the orphan row *shape* requirements are unchanged, so the orphan-claim flow and ACL referenceability guarantees carry over verbatim.

## Migration Plan

Single deploy of backend + frontend. No schema migration (the unique index on `eve_character_id` already exists). Rollback is a revert; orphans minted-on-add remain valid rows either way.
