## Why

The audit log cannot answer "Who added Wasp 222 to ACL test1?". The `acl_member_added` event stores only `member_id` (an internal `acl_member` UUID) — no member name — so the row is unreadable once that member is removed (the row is deleted) and the question is unanswerable even with a join. This is one instance of a systemic gap: several live events record bare IDs with no snapshotted name, and the audit search (`q`) only matches the actor and target-name columns, never the secondary entity a row is actually about.

The principle this change establishes: **the audit log SHALL be self-contained** — readable and searchable forever, independent of live state, because referenced entities (characters, accounts, maps, ACLs, ACL members) may later be deleted. Read-time resolution fails exactly when the audit log matters most: after the entity is gone.

## What Changes

- **Snapshot a human-readable name for every entity an event references.** The primary entity's name already goes in the `target_name` column; this change fills the gaps where it is left NULL, and adds *secondary* entity names to `details` (the ACL member, the second-of-two map/ACL in attach/detach, the old/new owner in ownership changes, the revoked key's label).
- **For ESI-resolved entities (characters/corps/alliances), store the EVE id alongside the name.** The EVE id is durable external identity; the internal `acl_member` UUID (`member_id`) is the weakest id — it is deleted on member-remove — and is **dropped**.
- **Drop `details` keys that merely duplicate `target_id`** (e.g. `acl_id`/`map_id` echoing the target column) — hygiene now that the indexed target column is the source of truth.
- **Extend audit search to match `details`.** `list_audit_log`'s `q` axis gains `OR details::text ILIKE '%q%'` alongside the existing actor/target-name match. This is a deliberate, flat substring search — no `search_text` column, no migration, no Postgres extension, no index — appropriate at wormhole scale. Matching a term that appears in a `permission`/`source` value is correct behaviour, not noise. **BREAKING** relative to the `audit-log` capability's current assertion that `details()` shapes are frozen and that `q` searches only actor/target names.
- **Surface details in the audit browser UI.** Add a per-row "Details" affordance that opens a dialog rendering `entry.details` as a generic key/value list. Pure render — no ID resolution. The dialog is only useful because the names are now snapshotted into `details`.
- No backfill: pre-existing rows keep their bare IDs and partial searchability; only rows written after this change are fully named and searchable.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `audit-log`: The `AuditEvent` catalogue's `details()` payloads change (names + EVE ids added to several variants; duplicate/internal ids removed) — overturning the current "`details()` shapes SHALL be unchanged" assertion. The `target()` mapping fills `target_name` for character-target events currently marked "name not carried (NULL)". The `list_audit_log` `q` requirement extends to match `details::text`.
- `server-administration`: The audit-browser UI requirement gains a per-row Details affordance opening a key/value dialog over `entry.details`.

## Impact

- **Backend** (`backend/src/audit/mod.rs`): `AuditEvent` variant fields, `details()`, `target()`, and the `q` clause in `list_audit_log`; stale "Dormant:" doc-comments on now-live variants removed.
- **Backend services** (emit sites): `services/acl.rs` (ACL member trio), `services/map.rs` (attach/detach), `services/admin.rs` (ownership changes, block/unblock), `services/account.rs` (character remove/set-main), `services/api_keys.rs` (key revoke), `services/auth.rs` + `services/token_sweep.rs` (character-subject events) — each must resolve and pass the name(s) it does not currently hold.
- **Backend db** (`backend/src/db/acl_member.rs`): `remove_member` returns the removed member's name instead of `bool`; `db/map_acl.rs` detach similarly may need to return names not currently loaded.
- **Frontend** (`frontend/src/routes/admin/audit/+page.svelte`): Details dialog + i18n chrome keys (en/de/fr).
- **No schema migration.** `details::text` search uses existing columns; no new column, index, or extension.
- **No backfill.** Historical rows remain as-is.
