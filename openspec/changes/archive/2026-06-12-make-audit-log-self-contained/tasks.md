## 1. DB layer — return names the emit sites need

- [x] 1.1 `db/acl_member.rs::remove_member`: change `-> Result<bool>` to return the removed member's name (e.g. `-> Result<Option<AclMember>>` or `Option<String>`) via `RETURNING name`; `None` means not-found. Update the existing `remove_member_deletes` test.
- [x] 1.2 `db/map_acl.rs`: ensure attach/detach paths can supply the **map** name to the service (load it, or return it from the detach helper). The ACL name is already available on attach via `find_acl_by_id`. *(Decision: map name loaded at the service layer via existing `db::map::find_map_by_id`; no `map_acl.rs` change needed.)*
- [x] 1.3 `db/api_keys` delete path (`delete_for_account_in_tx`): return the deleted key's `name` (`RETURNING name`) so `delete_key` can snapshot the label.

## 2. Audit core — `backend/src/audit/mod.rs`

- [x] 2.1 Add the fields needed for self-contained naming to the relevant `AuditEvent` variants: `member_name` + `eve_entity_id` on the ACL-member trio (drop the internal `member_id`); `map_name`/`map_id` on `AclAttachedToMap`/`AclDetachedFromMap`; `old_owner_name`/`new_owner_name` on `AdminMapOwnershipChanged`/`AdminAclOwnershipChanged`; `key_name` on `ApiKeyRevoked`; carried `character_name` on the character-subject variants (`CharacterRemoved`, `CharacterSetMain`, `EveCharacterBlocked`, `EveCharacterUnblocked`, `BlockedLoginRejected`, `CharacterOwnerMismatch`).
- [x] 2.2 Update `details()` per the `audit-log` delta: ACL-member trio → `{member_name, [member_type], [permission], eve_entity_id}`; attach/detach → `{map_name, map_id}` (acl is the target, named via `target_name`); ownership → `{old_owner_name, old_owner, new_owner_name, new_owner}`; `ApiKeyRevoked` → `{key_name}`. Remove keys that duplicate `target_id` (`acl_id`, `map_id` where it echoes the target).
- [x] 2.3 Update `target()` so the character-subject variants carry a name (`AuditTarget::character(id, Some(name))`) instead of `None`, populating `target_name`.
- [x] 2.4 Remove the now-stale `/// Dormant:` doc-comments from variants that are emitted by live code (ACL trio, attach/detach, map/acl create/delete/rename, ownership, etc.).
- [x] 2.5 Confirm `record_in_tx` needs no change beyond compiling against the new variant fields (actor + account-target name resolution stays as-is; secondary names arrive pre-resolved from the services).

## 3. Emit sites — resolve and pass names

- [x] 3.1 `services/acl.rs::add_member`: pass `member.name` and the member's EVE id into `AclMemberAdded`.
- [x] 3.2 `services/acl.rs::update_member_permission`: pass `updated.name` and EVE id into `AclMemberPermissionChanged`.
- [x] 3.3 `services/acl.rs::remove_member`: use the name returned by 1.1 in `AclMemberRemoved`; keep `None → NotFound`.
- [x] 3.4 `services/map.rs::attach_acl_to_map` / `detach_acl_from_map`: load the map name and pass it into the attach/detach events.
- [x] 3.5 `services/admin.rs` ownership-change handlers: resolve old/new owner accounts to their main character names and pass them into the ownership events. *(No live emit site exists — `AdminMapOwnershipChanged`/`AdminAclOwnershipChanged` remain dormant; the variants now carry `old_owner_name`/`new_owner_name: Option<String>` so a future handler supplies them.)*
- [x] 3.6 `services/admin.rs` block/unblock + `services/account.rs` (`CharacterRemoved`/`CharacterSetMain`) + `services/auth.rs` (`BlockedLoginRejected`) + `services/token_sweep.rs` (`CharacterOwnerMismatch`): pass the character name where in hand, or look it up. Block passes its `character_name` param; unblock returns it from `delete_block`'s `RETURNING`; account sites get it from the extended `lookup_for_account`; auth uses `input.character_name`; sweep uses the extended `RefreshableCharacter.name`. SSO-boundary block/unblock/rejected carry `Option<String>` (NULL when the name is genuinely unavailable).
- [x] 3.7 `services/api_keys.rs::delete_key`: pass the key label (from 1.3) into `ApiKeyRevoked`.

## 4. Search — extend `q` into `details`

- [x] 4.1 In `list_audit_log`, extend the `q` clause to `(actor_character_name ILIKE $q OR target_name ILIKE $q OR details::text ILIKE $q)`, reusing the existing literal-escaping for the bound fragment. No new column, index, or extension.

## 5. Backend tests

- [x] 5.1 Unit tests for the changed `details()` shapes (ACL trio incl. `eve_entity_id`, attach/detach, ownership, `ApiKeyRevoked`) and the `target()` name change for character-subject variants.
- [x] 5.2 Update/extend `list_audit_log` tests: `q` matches a name present only in `details` (the "Wasp 222" case); `q` still matches actor/target; metacharacters escaped; `q` combines conjunctively with `event_type`/`since`; `q` matches a non-name details value (the accepted trade-off). The existing conjunction test already seeds via the actor-name axis, which is unchanged.
- [x] 5.3 Service/integration tests for the emit sites that gained a name lookup (`remove_member` name in `services/acl.rs`; `character_name`/`key_name` snapshots in `tests/audit_log.rs`).
- [x] 5.4 Add/extend a HURL test under `tests/hurl/` exercising audit search over a value carried only in `details` (`q=hurl-smoke` against the block rows, in `admin.hurl`).
- [x] 5.5 `cargo test` (workspace) and `cargo clippy -- -D warnings` pass.

## 6. Frontend — Details dialog (`frontend/src/routes/admin/audit/+page.svelte`)

- [x] 6.1 Add a per-row "Details" affordance that opens a `<dialog>` rendering `entry.details` as a generic key/value list (`Object.entries`), with an empty-state for `{}`. Pure render — no id resolution, no mutation, dismissable. *(Extracted into a reusable `AuditDetailsDialog.svelte` for isolated component testing.)*
- [x] 6.2 Add i18n message keys for the dialog chrome (title, close, empty) and keep en/de/fr in lockstep per project convention.
- [x] 6.3 Vitest component tests: dialog opens with key/value rows, renders `member_name`, handles empty details, closes without side effects.
- [x] 6.4 Playwright e2e: open a row's Details, assert content visible, close.

## 7. Verification (all must pass before commit)

- [x] 7.1 `cargo test` and `cargo clippy --all-targets -- -D warnings` (backend) pass (376 lib + all integration suites green). The live HURL audit-search step (`admin.hurl` step 21, `q=hurl-smoke` over `details::text`) is operator-run against a live admin-session stack.
- [x] 7.2 `pnpm --filter frontend test` — Vitest unit/component tests pass (290 tests; run from `frontend/` per project convention, not `--filter` from repo root which has no root manifest).
- [x] 7.3 `pnpm --filter frontend run check` — svelte-check passes (0 errors / 0 warnings).
- [x] 7.4 `pnpm --filter frontend run test:e2e` — Playwright e2e tests pass (25 tests).
