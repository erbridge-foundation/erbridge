## Context

The `audit-log` capability already snapshots the **actor** (account + main character EVE id + name) and **target** (`target_type`/`target_id`/`target_name`) so attribution survives account hard-deletes and character renames. But the per-variant `details()` payloads are inconsistent: single-entity events put the name in `target_name`, while events that reference a *secondary* entity store only its id with no name. The trigger case is `acl_member_added`, whose `details` is `{acl_id, member_id, member_type, permission}` — `acl_id` duplicates `target_id`, and `member_id` is an internal `acl_member` row UUID that is deleted when the member is removed. There is no member name anywhere, so "Who added Wasp 222?" cannot be answered.

A codebase sweep found three name-missing clusters across live emit sites:

1. **Bare `eve_character_id`, `target_name=None`** — `CharacterRemoved`, `CharacterSetMain`, `EveCharacterBlocked`, `EveCharacterUnblocked`, `BlockedLoginRejected`, `CharacterOwnerMismatch`. The `target()` mapping deliberately uses `character(id, None)` for these; under the self-contained principle that is a latent bug, because character names resolve only while the character row exists.
2. **ACL-member trio bare `member_id`** — `AclMemberAdded`, `AclMemberPermissionChanged`, `AclMemberRemoved`.
3. **Relationship/ownership UUIDs** — `AclAttachedToMap`, `AclDetachedFromMap` (two first-class entities, only one target slot), `AdminMapOwnershipChanged`, `AdminAclOwnershipChanged` (old/new owner accounts), plus `ApiKeyRevoked` (no key label snapshotted).

Search compounds the gap: `list_audit_log`'s `q` matches only `actor_character_name` and `target_name`. Even once a member name is snapshotted into `details`, `q` would not find it.

## Goals / Non-Goals

**Goals:**
- Every audit event snapshots a human-readable **name** for each entity it references, at write time, so rows stay readable after the entity is deleted.
- ESI-resolved entities (character/corp/alliance) carry the durable **EVE id** alongside the name.
- The audit search (`q`) can find any snapshotted name, including secondary entities — answering "Who added Wasp 222 to test1?".
- The audit browser surfaces `details` to the admin.

**Non-Goals:**
- No read-time resolution of ids → names anywhere (the explicit anti-pattern this change forbids).
- No `search_text` column, GIN/trigram index, `pg_trgm`, or any schema migration — flat substring search is sufficient at wormhole scale, consistent with `redesign-audit-filtering`.
- No backfill of pre-existing rows.
- No change to the actor snapshot, the target columns' schema, keyset pagination, time-window/`since` semantics, or the `INSERT-only` invariant.

## Decisions

### D1: Snapshot names at write time; never resolve at read

A row like "removed member 9474cdcb…" becomes permanently unanswerable once the member/character/map/account is deleted — which is precisely when an audit log earns its keep. So the name is captured at write time and stored. This also removes the temptation to "just join on the id later," re-introducing the fragility deletion breaks.

*Alternative considered — resolve ids to names when rendering the Details dialog.* Rejected: it is correct only while the referenced row exists, and the audit log's whole purpose is to outlive live state.

### D2: Name placement — primary in `target_name`, secondary in `details`

The `redesign-audit-filtering` change already promoted the primary entity's identity out of `details` into the indexed `target_*` columns. This change completes that move: the primary entity's name belongs in `target_name` (fill the `character(id, None)` gaps), and `details` carries names only for *secondary* entities the target columns cannot hold (the member; the non-target one of a map/ACL pair; old/new owners; the key label). Duplicate primary ids in `details` (`acl_id`, `map_id` echoing `target_id`) are removed.

### D3: ESI entities store `eve_id` + name; drop the internal member UUID

For an ACL member that is a character/corp/alliance, store `eve_entity_id` (or `eve_character_id`) + `name`. The EVE id is global, stable, and the durable join key to ESI. The internal `acl_member` row UUID (`member_id`) is the weakest identity — it is deleted on remove — and is dropped. (If cross-row correlation of a single `acl_member` row's lifetime is ever wanted, it can be reintroduced; it is not needed today.)

### D4: Flat `details::text` search, not a search column

`q` gains one clause: `OR details::text ILIKE '%fragment%'` (same literal-escaping as the existing axes), so it matches `actor_character_name OR target_name OR details::text`. This is the cheapest possible exposure of the snapshotted names — no new column, no migration, no index, no extension. It is the name-snapshotting (D1–D3) that makes search *work*; flat search merely surfaces it.

*Alternatives considered:* (a) a top-level `search_text` column populated from a per-variant `search_names()` method — rejected as over-engineering for wormhole scale, and it would force the Details dialog to special-case-hide the field; (b) a `_search` magic key inside `details` — same dialog-noise problem; (c) per-key OR list (`details->>'member_name' OR …`) — brittle, every new event must remember to extend the query. Flat `details::text` is zero-maintenance and self-completing: any name snapshotted into `details` is searchable the instant it is written.

*Accepted trade-off:* `details::text` matches the JSON text including keys, braces, and non-name scalar values (`permission`, `member_type`, `source`). At this scale that is acceptable — a user typing "admin" reasonably expects rows where "admin" appears. If values-only matching is ever wanted, `jsonb_each_text` is a drop-in upgrade.

### D5: Name availability at emit sites varies; resolve at the service layer

Where the name is already in scope, thread it (cheap): `acl::add_member` has `input.name`; `update_member_permission` already binds the updated row (`updated.name`). Where it is not, the service performs the lookup before recording (not `record_in_tx`, which stays simple — it only resolves actor + account-target names, as today):
- `acl::remove_member`: change `db::acl_member::remove_member` from `-> bool` to `-> Option<member>` (or `Option<String>` name) via `RETURNING name`; `None` preserves today's `NotFound` semantics.
- `map::attach_acl_to_map`/`detach_acl_from_map`: load the map name (the ACL name is already loaded on attach) before recording.
- `admin` ownership changes: resolve old/new owner accounts to their main character names (the same `AccountMain` notion the target column already uses) at the service layer and pass plain strings.
- `api_keys::delete_key`: have the delete return the key label (`RETURNING name`) before recording.
- character-subject events: pass the character name where the handler already has it; `token_sweep` (`CharacterOwnerMismatch`) looks it up.

### D6: Generic key/value Details dialog, no resolution

The audit browser gains a per-row "Details" affordance opening a `<dialog>` that renders `entry.details` as a key/value list (`Object.entries`, insertion order from `serde_json`). It performs no id resolution; it relies on the names being present (D1–D3). `details` is already on `AuditLogEntryDto` (`api.ts`), so this is frontend-only.

## Risks / Trade-offs

- **[Overturns a frozen-shape assertion]** The `audit-log` spec currently states `details()` shapes SHALL NOT change → this change deliberately revises that requirement and its scenarios; the delta spec must restate the new per-variant payloads explicitly.
- **[Partial searchability for old rows]** No backfill → pre-existing rows lack the new names and won't match `q` on them → accepted; the handful of existing rows is not worth a migration, and `details::text` still matches whatever ids they do contain.
- **[`details::text` performance]** Unindexed substring scan → mitigated by the time-window bound (`since`) that keeps the candidate set small, exactly as the existing `q`/`target_name` axes already rely on; consistent with `redesign-audit-filtering`'s deliberate no-index stance.
- **[Emit-site lookups add queries]** D5 introduces a few name lookups on write paths (ownership change, key revoke, member remove) → low frequency admin/owner actions; negligible, and they run inside the existing transaction.
- **[i18n drift]** New dialog chrome keys must be added to en/de/fr in lockstep (project convention) → covered explicitly in tasks and `pnpm run check` (paraglide compile).

## Migration Plan

No database migration. Deploy is code-only: backend `details()`/`target()`/`q` changes + service emit-site name resolution, then the frontend dialog. Rollback is a plain revert; no schema state to undo, and rows written under the new code remain valid JSON readable by old code (extra keys are ignored; the dialog simply renders fewer rows).

## Open Questions

None outstanding. (`details::text` vs values-only, the `search_text` column, and id-removal scope were resolved during exploration in favour of the decisions above.)
