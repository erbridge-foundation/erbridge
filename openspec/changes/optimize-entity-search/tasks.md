# Tasks — optimize-entity-search

## 1. Bulk name resolution

- [ ] 1.1 Add `resolve_names_bulk` in `esi/search.rs` calling `POST /universe/names/` once for all (capped) matched ids; partition by response `category`; treat 404-with-ids as all-dropped; delete `resolve_named`, `resolve_character_names`, `resolve_entity_names` per-id loops
- [ ] 1.2 Update `services/entity_search.rs` and the orphan-affiliation lookup to the bulk resolver (affiliation corp/alliance names resolve in the same style)
- [ ] 1.3 Wiremock tests: single bulk call asserted (request counting), partition correctness, omitted-id drop, 404 handling

## 2. Write-free search

- [ ] 2.1 Replace per-result `find_id_by_eve_character_id` + `find_or_mint_character` with one batched lookup (`WHERE eve_character_id = ANY($1)`) in `db/characters.rs`; `CharacterMatch.id` becomes `Option<Uuid>`; remove the search-path mint
- [ ] 2.2 Update `dto/entity.rs` (`characters[].id` nullable) and OpenAPI schema; integration test asserting a search inserts zero rows

## 3. Mint-on-add

- [ ] 3.1 Relax `validate_member_shape` in `services/acl.rs`: a character member no longer requires `character_id` (it stays required that every member carries `eve_entity_id`, and that corp/alliance members carry no `character_id`). No new request field — reuse the existing `eve_entity_id`/`character_id` on `AddMemberInput`/`AddMemberRequest`; update the doc-comments accordingly
- [ ] 3.2 In `services/acl::add_member`: when a character member arrives with no `character_id`, pre-fetch affiliations outside the tx; find-or-mint the orphan inside the tx keyed by `eve_entity_id` with `ON CONFLICT (eve_character_id) DO NOTHING` + re-select; insert the member referencing the resolved UUID. The `character_id`-present path is unchanged.
- [ ] 3.3 Tests: existing-row path (character_id present), mint path (character_id absent), reuse path (absent but row exists), missing-eve_entity_id rejection, corp/alliance-with-character_id rejection, concurrent mint race (unique-index arbitration), orphan shape assertions, claim-after-mint flow. Update the existing `validate_member_shape` unit tests that assert "character members require character_id"

## 4. Admin search batched blocked-check

- [ ] 4.1 Add `db/blocks::blocked_set(pool, &[i64]) -> HashSet<i64>`; use it in `services/admin::search_characters` and `esi_search_characters`; keep single-id check for the SSO callback
- [ ] 4.2 Tests: annotation correctness against a mixed blocked/unblocked result set; query-count assertion if practical

## 5. Frontend

- [ ] 5.1 `MemberPicker.svelte`: character rows always submit `eve_entity_id` (from `c.eve_character_id`) and submit `character_id` only when the search result carries a UUID (`c.id != null`); key character rows on `c.eve_character_id`, not the now-nullable `c.id`; `acls/[id]` `addMember` action forwards `character_id` only when present
- [ ] 5.2 Update `src/lib/api.ts` types (`EntityCharacterDto.id: string | null`; `AddMemberRequest.character_id` already optional — confirm and document the new mint-when-absent semantics); adjust Vitest suites and the e2e mock backend (unknown-character add sends no `character_id`)

## 6. Verification

- [ ] 6.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`; `cargo sqlx prepare -- --all-targets` and commit the cache diff
- [ ] 6.2 Update HURL: `entities.hurl` (nullable id), `acls.hurl` (add a character member with `eve_entity_id` and no `character_id`, asserting the orphan mint); live HURL run against dev compose
- [ ] 6.3 From `frontend/`: `pnpm test` — Vitest unit/component tests
- [ ] 6.4 From `frontend/`: `pnpm run check` — svelte-check (type checking + paraglide compile)
- [ ] 6.5 From `frontend/`: `pnpm run test:e2e` — Playwright e2e tests
- [ ] 6.6 Live smoke test: member-picker search for a never-seen pilot adds them successfully; confirm exactly one orphan row minted and only on add
