## Context

The codebase has accounts, characters (with ESI tokens and a `token_status` lifecycle), API keys, blocks, audit logging, and admin tooling — but no **map**, the domain object the whole product exists to produce. There is also no access-control model: today every authenticated account acts only on its own resources, gated by the `AuthenticatedAccount` extractor (per-account) or `AdminAccount` (server admin). Maps need something richer — a map is *shared*, and "who can see/edit this map" is a first-class question answered by reusable, named ACLs.

A full prior implementation exists at `zz-ref/backend/older-iteration/`. The relevant pieces:
- Migrations: `0005_create_maps_core.sql` (map + connections + ends + signatures), `0008_create_acl.sql`, `0009_create_acl_member.sql`, `0010_create_map_acl.sql`, `0012_acl_member_check.sql`.
- `src/db/{map,acl,acl_member,map_acl}.rs`, `src/dto/{map,acl}.rs`, `src/services/{map,acl}.rs`, `src/handlers/{map,acl}.rs`.
- `src/permissions.rs` — the `effective_permission` resolver and the ordered `Permission` enum.

The reference is a **reference, not a drop-in.** It predates the current codebase's conventions and carries subsystems we are not building. This change ports a deliberately narrow slice and adapts it.

**Convention deltas the port must honor (current codebase vs. reference):**

| Concern | Reference | Current codebase (authority: `rust-rest-api` skill) |
|---|---|---|
| Error type | per-service `MapError` / `AclError` enums, each `IntoResponse` | one `AppError` in `error.rs` + `ConflictKind`; handlers map service errors to it |
| Envelope | `ApiResponse::ok(..)` / `::error(..)` | `ApiResponse::data(..)` (data-only envelope) |
| Account extractor | `AccountId(Uuid)` | `AuthenticatedAccount(Uuid)` (and `AdminAccount(Uuid)`) in `handlers/middleware.rs` |
| Module naming | `handlers/map.rs` | `handlers/api/v1/maps.rs` (route groups live under `api/v1/`) |
| Migration naming | `0005_*`, `0008_*` … | `0000000000000N_*` (next is `00000000000009_*`) |
| Map soft-delete | `deleted BOOLEAN` | `status TEXT DEFAULT 'active'` + `delete_requested_at` (mirrors `account`) |
| ACL lifecycle | `pending_delete_at` + grace-period purge job | dropped — plain CRUD, no reaper |

## Goals / Non-Goals

**Goals:**
- Create the four foundational tables — `map`, `acl`, `acl_member`, `map_acl` — with FK and CHECK constraints, in the current migration style.
- Ship account-scoped CRUD over maps and ACLs, plus map↔ACL attach/detach.
- Port the **permission resolver** so attached ACLs actually grant resolved access to maps (`read`/`read_write`/`manage`/`admin`, with `deny` override and most-permissive-wins), and gate every map operation through it.
- Record mutations in the existing audit log.

**Non-Goals:**
- No map contents (connections, signatures, routes, the edges view).
- No event-sourcing (`map_event`, checkpoints, the checkpoint task).
- No ACL orphan-reaping / purge job (the reference's `pending_delete_at` mechanism).
- No frontend (a later change), and no admin-wide listing endpoints.

## Decisions

### Port the permission resolver now; gate every map op through it
The four tables are inert without a resolver: `acl_member` rows only *mean* something when something reads them to make an authz decision. Shipping the schema + CRUD but gating maps owner-only would leave ACL membership stored-but-unenforced — a half-built feature that invites bugs (members appear to grant access in the UI but don't). So the resolver is in-scope.

The ported resolver (`permissions.rs::effective_permission`) does exactly two queries:
1. **Owner check** — `SELECT EXISTS(... FROM map WHERE id = $1 AND owner_account_id = $2 AND status = 'active')`. Owner ⇒ `Admin`. *(Adapted: the reference checked `deleted = false`; the current convention is `status = 'active'`.)*
2. **Member match** — join `map_acl → acl_member → eve_character (account_id = $2)`, matching on direct character (`character_id = ec.id`), corporation (`eve_entity_id = ec.corporation_id`), or alliance (`eve_entity_id = ec.alliance_id`, non-null). Collect all matched permissions.

Then: any `deny` ⇒ hard stop (`None`); otherwise the most-permissive grant wins (`Permission` derives `Ord`). `deny` parses *outside* the `Permission` enum on purpose (the enum is `Read/ReadWrite/Manage/Admin`); the resolver treats a `deny` string as the veto, never as a grant.

*Alternative considered — owner-only now, resolver later.* Rejected during exploration: it ships inert tables and a misleading member UI.

### Soft-delete: mirror `account` for maps; no soft-delete machinery for ACLs
Maps get `status TEXT NOT NULL DEFAULT 'active'` + `delete_requested_at TIMESTAMPTZ`, the same shape as the `account` table, so soft-delete reads consistently across the schema and the resolver's owner check naturally filters on `status = 'active'`. (Maps have only two meaningful states — live and deleted — so the `status` column starts as a two-value field; using the same column *name/shape* as `account` is the consistency win, not a claim that maps need account's full lifecycle.)

ACLs get **no** soft-delete column. The reference's `pending_delete_at` was not a soft-delete flag at all — it drove orphan-reaping (an ACL with no attached maps was marked pending and a background job purged it after a grace period, ADR-028). We are **dropping orphan-reaping** (see below), so the column has no purpose. An ACL is created, lives, and is explicitly deleted (FK cascades remove its members and `map_acl` rows).

### Drop ACL orphan-reaping; surface "unattached" in the UI later instead
Reaping pulls in a background job, and the jobs interface is deliberately deferred (only the token sweep exists today). It also contradicts the account-style soft-delete decision. Dropping it means a detached ACL simply persists — which is *fine*; an ACL with no map is harmless data. The *signal* the reaper provided ("this ACL guards nothing") is still useful, so the future frontend change should render an **"unattached ACL" indicator** for user reference. We keep the signal, drop the automation.

### Strip the event-sourcing interleave from the ported service
The reference's `services/map.rs` interleaves two concerns inside each transaction: `audit::record_in_tx(...)` (kept) and `map_event::append_event(...)` (the cut event log). The port keeps the audit calls verbatim in spirit and **removes every `map_event::append_event` call**. This is the main porting hazard — the two are adjacent in the source and easy to copy together. The service after porting touches `audit` only; there is no `map_event` module in this change.

### One `AppError`, not per-service error enums
The reference defines `MapError` and `AclError`, each with its own `IntoResponse`. The current codebase has a single `AppError` (in `error.rs`) with a `ConflictKind` sub-enum, mapped to HTTP at the handler boundary. The port folds the reference's error cases into `AppError`:
- slug collision ⇒ a new `ConflictKind` (e.g. `MapSlugAlreadyExists`) ⇒ 409/422 per the existing conflict convention.
- ACL-owner mismatch on attach, and resolver "no access" ⇒ the existing `AppError::Forbidden` (403).
- not-found ⇒ existing `AppError::NotFound` (404).

Services return service-level errors / `AppError` per the codebase's existing pattern; handlers do the mapping. No new `IntoResponse` impls beyond what `error.rs` already provides.

### `acl_member` keeps the denormalized `name` and the role-for-type CHECK
- `name TEXT NOT NULL DEFAULT ''` snapshots the entity's display name (e.g. corp name) so list/resolver responses don't re-resolve `eve_entity_id` against ESI on every read. Cheap, and the UI wants it.
- `member_type = 'character' OR permission NOT IN ('manage','admin')` encodes real intent: a corporation or alliance can be *granted access* to a map (read/read_write/deny) but cannot *administer* the ACL itself — only a named character can hold `manage`/`admin`. One-line CHECK, kept.

### Two capabilities, one change
`maps` and `acls` are split into two capabilities (matching the reference's two route groups and the natural spec boundary), but delivered in **one change** because they are interdependent: `map_acl` is the join, attach/detach lives on the maps surface but validates ACL ownership, and the resolver reads ACL members to gate maps. Splitting the *change* would force an awkward "acls land first, inert" intermediate state. One reviewable unit.

### Map slug is globally unique
The `slug` column is `UNIQUE` across the whole `map` table — not scoped per owner. Two different owners cannot both hold a `home` slug; the first to take it wins, and a collision surfaces as a typed conflict (`ConflictKind::MapSlugAlreadyExists`), not a 500. This matches the reference and treats the slug as a global, URL-addressable handle for a map rather than an owner-private label. A collision is the creator's signal to pick another slug.

## Risks / Open Questions

- **`character_id` vs `eve_entity_id` for character members.** The schema allows a character member to carry both a `character_id` (FK to `eve_character.id`) and an `eve_entity_id`. The resolver matches characters on `character_id`. The member-add path must set `character_id` for `member_type = 'character'` and `eve_entity_id` for corp/alliance; enforcing "exactly the right column for the type" is service-layer validation (the CHECK constraints don't fully pin this). Worth an explicit service test.
- **Name snapshots go stale.** `acl_member.name` is a point-in-time snapshot; if a corp renames, the stored name drifts. Acceptable for v1 (display-only, re-derivable); a refresh path can come later.
- **No integration with `token_status`.** A map owner whose characters are all `token_expired` still owns the map (ownership is account-level, not token-gated). Correct for now; noted so it isn't mistaken for an oversight.
