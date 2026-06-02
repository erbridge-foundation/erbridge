## Why

The product is a wormhole mapper, but the core domain object — a **map** — does not yet exist, and neither does the access-control model that decides who may read or edit one. Everything shipped so far (accounts, characters, tokens, blocks, admin) is the substrate; the map is the thing the substrate exists to serve.

This change lays the **relational and authorization foundation** for maps: the `map` container, a reusable named **ACL** (access-control list) with members, and the join that attaches ACLs to maps — plus the resolver that turns "this account's characters, in these corps/alliances" into an effective permission on a given map. It deliberately stops short of the map's *contents*: no connections, signatures, routes, or event-sourcing. A map you can create, own, share via ACLs, and have access correctly resolved against — but not yet draw on.

A reference implementation of the full subsystem exists at `zz-ref/backend/older-iteration/` (migrations `0005`/`0008`/`0009`/`0010`/`0012`, plus `src/{db,dto,services,handlers}/{map,acl}*` and `src/permissions.rs`). This change ports the **container + ACL + resolver** slice of it, adapted to the current codebase's conventions (single `AppError`, `ApiResponse::data` envelope, `AuthenticatedAccount`/`AdminAccount` extractors, layer-named modules), and drops the reference's checkpoint/retention fields, its event-sourcing, and its ACL orphan-reaping job.

## What Changes

- **`map` table + CRUD.** Create, list (maps the account can read), get, update (name/slug/description), and **soft-delete** a map. Soft-delete mirrors the `account` convention: `status TEXT NOT NULL DEFAULT 'active'` + `delete_requested_at TIMESTAMPTZ`. Slug is unique; a slug collision is a typed conflict, not a 500.
- **`acl` + `acl_member` tables + CRUD.** Create, list (ACLs the account can manage), rename, and delete an ACL; add, list, update-permission, and remove members. A member is a character (by `eve_character.id`), a corporation, or an alliance (by `eve_entity_id`), each granted one of `read` / `read_write` / `manage` / `admin` / `deny`. `manage`/`admin` are reserved for character members (a corp/alliance can be granted access but cannot *administer* an ACL).
- **`map_acl` join + attach/detach.** Attach an ACL the caller owns to a map the caller administers; detach it. Many-to-many: an ACL can guard many maps, a map can be guarded by many ACLs.
- **The permission resolver (`effective_permission`).** Ported from the reference: map owner ⇒ `admin`; otherwise match the account's characters against every member of every ACL attached to the map (direct character, corporation, or alliance); a `deny` anywhere is a hard stop; otherwise the most-permissive grant wins. Every map read/write/admin operation is gated by this resolver, not by bare ownership.
- **Audit.** Map create/delete and ACL attach/detach (and ACL/member mutations) emit audit events through the existing audit module, in its established `kind`-string house style.

## Capabilities

### New Capabilities
- `maps`: the `map` container — its schema and soft-delete lifecycle, CRUD gated by the permission resolver, and ACL attach/detach. Includes the resolver itself (owner ⇒ admin; member match across attached ACLs; deny-overrides; most-permissive-wins) as the authority every map operation consults.
- `acls`: the reusable named ACL and its members — CRUD over `acl` and `acl_member`, the member-type/permission constraints, and the "manageable by this account" listing (owner or a character member holding `manage`/`admin`).

## Impact

- **Schema** (new migration `00000000000009_create_map_and_acl.sql`, following the current `0000000000000N` naming — not the reference's `0005`/`0008`–`0010`):
  - `map` — `id`, `name`, `slug UNIQUE`, `owner_account_id → account(id) ON DELETE SET NULL`, `description`, `status TEXT NOT NULL DEFAULT 'active'`, `delete_requested_at TIMESTAMPTZ`, `created_at`, `updated_at`; index on `owner_account_id`. **Dropped vs. reference:** `last_checkpoint_seq`, `last_checkpoint_at`, `retention_days` (the cut event/checkpoint subsystem). The reference's `map_connections` / `map_connection_ends` / `map_signatures` tables are **not** created.
  - `acl` — `id`, `name`, `owner_account_id → account(id) ON DELETE SET NULL`, `created_at`, `updated_at`. **Dropped vs. reference:** `pending_delete_at` (no orphan-reaping; see Non-goals).
  - `acl_member` — `id`, `acl_id → acl(id) ON DELETE CASCADE`, `member_type`, `eve_entity_id BIGINT`, `character_id → eve_character(id) ON DELETE CASCADE`, `name TEXT NOT NULL DEFAULT ''`, `permission`, timestamps. CHECKs: `member_type ∈ {character,corporation,alliance}`; `permission ∈ {read,read_write,manage,admin,deny}`; `member_type = 'character' OR permission NOT IN ('manage','admin')`.
  - `map_acl` — `map_id → map(id) ON DELETE CASCADE`, `acl_id → acl(id) ON DELETE CASCADE`, `created_at`, `PRIMARY KEY (map_id, acl_id)`.
- **Backend (Rust)** — per the `rust-rest-api` layer-named layout:
  - `db/map.rs`, `db/acl.rs`, `db/acl_member.rs`, `db/map_acl.rs` — domain structs + queries (insert/find/list/update/soft-delete map; ACL + member CRUD; attach/detach; the manageable-ACLs and maps-for-account listing queries).
  - `permissions.rs` (new top-level module, mirroring the reference) — the `Permission` enum (`Read < ReadWrite < Manage < Admin`, ordered) and `effective_permission(pool, account_id, map_id)`. The owner check reads `status = 'active'` (the current soft-delete convention), **not** the reference's `deleted = false`.
  - `services/map.rs`, `services/acl.rs` — business logic and orchestration; gate every map operation through `effective_permission`; gate ACL attach on ACL ownership. The reference interleaves `map_event::append_event` calls with audit in the same transactions — those event-sourcing calls are **stripped**; only audit is wired.
  - `handlers/api/v1/maps.rs`, `handlers/api/v1/acls.rs` — the two route groups (see surface below), using the `AuthenticatedAccount` extractor and returning DTOs in the `ApiResponse::data` envelope.
  - `dto/map.rs`, `dto/acl.rs` — request/response DTOs with `From<DbModel>` impls; `MapResponse` carries the attached-ACL summaries.
  - `error.rs` — add the conflict/forbidden cases this surface needs (slug conflict, ACL-owner mismatch) as `AppError`/`ConflictKind` variants in the existing single-enum style — **not** per-service error enums like the reference's `MapError`/`AclError`.
  - `audit/mod.rs` — audit variants for `MapCreated`, `MapDeleted`, `AclAttachedToMap`, `AclDetachedFromMap` (and ACL/member mutations) in the existing `kind`-string house style.
  - `main.rs` / router wiring — mount the two route groups under `/api/v1`.
- **API surface** (all under `/api/v1`, `AuthenticatedAccount`-gated):
  - acls: `GET /acls`, `POST /acls`, `PATCH /acls/{acl_id}`, `DELETE /acls/{acl_id}`, `GET /acls/{acl_id}/members`, `POST /acls/{acl_id}/members`, `PATCH /acls/{acl_id}/members/{member_id}`, `DELETE /acls/{acl_id}/members/{member_id}`.
  - maps: `GET /maps`, `POST /maps`, `GET /maps/{map_id}`, `PATCH /maps/{map_id}`, `DELETE /maps/{map_id}`, `POST /maps/{map_id}/acls`, `DELETE /maps/{map_id}/acls/{acl_id}`.
- **Frontend**: none in this change — backend-only. (A future frontend change builds the map/ACL UI; per the exploration it should include an "unattached ACL" indicator — an ACL attached to no map — for user reference, replacing the reference's automatic orphan-reaping.)
- **Tooling**: schema/query changes require regenerating the sqlx offline cache (`cargo sqlx prepare -- --all-targets` from `backend/`) and committing the `.sqlx/` diff.

## Non-goals

- **No map contents.** Connections, connection-ends, signatures, route-finding, the `system_edges` view — all deferred to a later change.
- **No event-sourcing.** The reference's `map_event` append-log, checkpoints, and the `tasks/map_checkpoint` job are not ported. Map/ACL mutations are recorded in the existing audit log only.
- **No ACL orphan-reaping.** The reference's `pending_delete_at` + grace-period purge job (ADR-028) is dropped; it contradicts the chosen account-style soft-delete and pulls in a background job that is otherwise deferred. An ACL attached to no map simply persists; surfacing that state is a future frontend concern, not a reaper.
- **No admin-wide ACL/map listing endpoints.** The reference's `find_all_acls_admin` and admin map views are out of scope; this change ships the account-scoped, resolver-gated surface only.
- **No frontend.** This change is backend-only.
