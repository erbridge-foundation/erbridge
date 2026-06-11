# Tasks — optimize-entity-search

## 1. Bulk name resolution

- [ ] 1.1 Add `resolve_names_bulk` in `esi/search.rs` calling `POST /universe/names/` once for all (capped) matched ids; partition by response `category`; treat 404-with-ids as all-dropped; delete `resolve_named`, `resolve_character_names`, `resolve_entity_names` per-id loops
- [ ] 1.2 Update `services/entity_search.rs` and the orphan-affiliation lookup to the bulk resolver (affiliation corp/alliance names resolve in the same style)
- [ ] 1.3 Wiremock tests: single bulk call asserted (request counting), partition correctness, omitted-id drop, 404 handling

## 2. Write-free search

- [ ] 2.1 Replace per-result `find_id_by_eve_character_id` + `find_or_mint_character` with one batched lookup (`WHERE eve_character_id = ANY($1)`) in `db/characters.rs`; `CharacterMatch.id` becomes `Option<Uuid>`; remove the search-path mint
- [ ] 2.2 Update `dto/entity.rs` (`characters[].id` nullable) and OpenAPI schema; integration test asserting a search inserts zero rows

## 3. Mint-on-add

- [ ] 3.1 Extend `AddMemberInput`/`AddMemberRequest` with `eve_character_id: Option<i64>` for character members; shape validation (exactly one of `character_id`/`eve_character_id`)
- [ ] 3.2 In `services/acl::add_member`: pre-fetch affiliations outside the tx when minting; find-or-mint inside the tx with `ON CONFLICT (eve_character_id) DO NOTHING` + re-select; insert member referencing the resolved UUID
- [ ] 3.3 Tests: mint path, reuse path, both/neither rejection, concurrent mint race (unique-index arbitration), orphan shape assertions, claim-after-mint flow

## 4. Admin search batched blocked-check

- [ ] 4.1 Add `db/blocks::blocked_set(pool, &[i64]) -> HashSet<i64>`; use it in `services/admin::search_characters` and `esi_search_characters`; keep single-id check for the SSO callback
- [ ] 4.2 Tests: annotation correctness against a mixed blocked/unblocked result set; query-count assertion if practical

## 5. Frontend

- [ ] 5.1 `MemberPicker.svelte`: character rows submit `character_id` when the UUID is present, else `eve_character_id`; `acls/[id]` `addMember` action validates and forwards accordingly
- [ ] 5.2 Update `src/lib/api.ts` types (`EntityCharacterDto.id: string | null`, `AddMemberRequest.eve_character_id`); adjust Vitest suites and the e2e mock backend

## 6. Verification

- [ ] 6.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`; `cargo sqlx prepare -- --all-targets` and commit the cache diff
- [ ] 6.2 Update HURL: `entities.hurl` (nullable id), `acls.hurl` (add-by-eve_character_id); live HURL run against dev compose
- [ ] 6.3 `pnpm --filter frontend test` — Vitest unit/component tests
- [ ] 6.4 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile)
- [ ] 6.5 `pnpm --filter frontend run test:e2e` — Playwright e2e tests
- [ ] 6.6 Live smoke test: member-picker search for a never-seen pilot adds them successfully; confirm exactly one orphan row minted and only on add
