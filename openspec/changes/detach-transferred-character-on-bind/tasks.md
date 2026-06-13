## 1. Schema & migration

- [ ] 1.1 New migration: add `account.last_known_main_character_id BIGINT NULL` and `account.last_known_main_character_name TEXT NULL` (NOT foreign keys).
- [ ] 1.2 Same migration: extend the `account.status` CHECK/domain to include `'orphaned'`.
- [ ] 1.3 Same migration: one-time backfill of `last_known_main_*` for existing accounts from their current `is_main = TRUE` character.
- [ ] 1.4 Regenerate the sqlx offline cache (`cargo sqlx prepare`) and commit the `.sqlx/` diff after all backend query changes land.

## 2. db layer — main snapshot invariant

- [ ] 2.1 `db/characters.rs::set_main` — within the same tx, write the owning account's `last_known_main_character_id` + `last_known_main_character_name` from the promoted row.
- [ ] 2.2 `db/characters.rs::promote_if_no_main` — accept the promoted character's `eve_character_id` + name from the caller and, when it promotes, write the account's `last_known_main_*` in the same tx; update all call sites to pass these values.
- [ ] 2.3 Add a `db/accounts.rs` helper (or extend existing) to set `last_known_main_*` for an account in-tx, reused by 2.1/2.2 and the seller re-promotion.
- [ ] 2.4 Unit tests: `set_main` and `promote_if_no_main` both update the snapshot; snapshot persists after the character is detached.

## 3. db layer — detach / seller fixup / orphan

- [ ] 3.1 `db/characters.rs` — detach-and-rebind query: `UPDATE eve_character SET account_id = <dest>, tokens/owner_hash/scopes/public-info, token_status='valid', updated_at=now()` for the transferred character.
- [ ] 3.2 `db/characters.rs` — remaining-character count for an account (to drive the seller fixup) and a "promote any remaining character to main" helper (updating the snapshot via 2.3).
- [ ] 3.3 `db/accounts.rs` — transition an account to `status = 'orphaned'` in-tx.
- [ ] 3.4 Unit tests for 3.1–3.3, including: detach leaves seller with >0 chars (main intact / main re-promoted) and detach empties seller (→ orphaned, row retained).

## 4. Service — transfer detection in complete_sso_callback

- [ ] 4.1 In `services/auth.rs`, before the resolve/bind decision, look up the existing row's `(account_id, owner_hash)` for the `eve_character_id` and compute the transfer predicate: presented hash present AND stored hash non-null AND differing.
- [ ] 4.2 Add a transferred branch covering BOTH login and add-character: rebind the character to the destination (resolved/new account for login; session account for add-character), run the seller-side fixup (re-promote or orphan), all in the existing SSO-completion transaction.
- [ ] 4.3 Ensure the add-character `BoundElsewhere` rejection is bypassed when the character is transferred, and preserved when it is not (absent hash / null stored / matching).
- [ ] 4.4 Ensure a matching-hash same-account login remains the existing normal self-heal (no detach, no transfer event).
- [ ] 4.5 `audit/mod.rs` — add a `CharacterTransferred` variant (kind-string house style) carrying destination as actor and snapshotting the former account id + `last_known_main_character_name`; emit it in the transferred branch.
- [ ] 4.6 Service-level tests: login-transfer (lands in fresh account, not seller's), add-character transfer (rebinds to session account), seller orphaned when emptied, seller main re-promoted when stripped, conservative fallback on absent/null/matching hash.

## 5. Admin hard-delete + deletion preview

- [ ] 5.1 `db/accounts.rs` — hard-delete (`DELETE FROM account WHERE id = $1`) relying on the existing FK CASCADE/SET-NULL behaviour; evaluate the last-server-admin guard in the same tx (HTTP 409 `cannot_remove_last_server_admin`).
- [ ] 5.2 `db/accounts.rs` — blast-radius counts for the preview: characters, sessions, API keys (to be removed); owned maps, owned ACLs (to become unowned).
- [ ] 5.3 `services/admin.rs` (+ `services/account.rs` as appropriate) — hard-delete service that returns the preview counts and performs the delete; emit an `AccountHardDeleted` audit variant carrying the deleted account id + `last_known_main_character_name`.
- [ ] 5.4 `handlers/api/v1/admin.rs` — endpoint(s) behind `AdminAccount`: preview (counts) and execute (delete). Wire routes + openapi.
- [ ] 5.5 Backend tests: admin-only gating (fail-closed), last-admin guard, cascade removes private rows, SET NULL preserves maps/ACLs/audit, audit event emitted.

## 6. Frontend — admin views

- [ ] 6.1 Surface orphaned accounts in the admin account view; render them nameable via `last_known_main_character_name`; ensure owner-less maps/ACLs remain visible.
- [ ] 6.2 Admin hard-delete UI: fetch + render the deletion preview (removed counts + maps/ACLs going unowned), with copy that states audit history is preserved (not lost), behind an explicit "this cannot be undone" confirmation before dispatch.
- [ ] 6.3 i18n: add keys for orphaned-status label, the deletion preview, and the confirm dialog across all locales (en/de/fr) per the project's locale-sync rule.
- [ ] 6.4 Frontend component/unit tests for the preview + confirm flow and the orphaned-account display.

## 7. Verification

- [ ] 7.1 Backend: `cargo test` (unit + integration) and `cargo clippy` clean; live HURL coverage for the new admin endpoints and the transfer/orphan flows where applicable.
- [ ] 7.2 Frontend (run from `frontend/`): `pnpm test` (Vitest).
- [ ] 7.3 Frontend (run from `frontend/`): `pnpm run check` (svelte-check + paraglide compile).
- [ ] 7.4 Frontend (run from `frontend/`): `pnpm run test:e2e` (Playwright).
- [ ] 7.5 Confirm the sqlx offline cache (`.sqlx/`) is regenerated and committed; all three frontend commands and the backend suite pass before the change is marked complete.
