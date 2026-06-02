# Tasks — add-maps-and-acls

> Authority: the `rust-rest-api` skill defines the module layout. Paths below follow it (layer-named modules; route groups under `handlers/api/v1/`). The reference at `zz-ref/backend/older-iteration/` is a source to adapt, not copy — honor the convention deltas in `design.md` (single `AppError`, `ApiResponse::data`, `AuthenticatedAccount`, `status`-based soft-delete, no `map_event`, no `pending_delete_at`).
> Backend-only change: the frontend verification trio does **not** apply.

## 1. Schema

- [x] 1.1 Add migration `backend/migrations/00000000000009_create_map_and_acl.sql` creating, in dependency order:
  - `map` — `id UUID PK DEFAULT gen_random_uuid()`, `name TEXT NOT NULL`, `slug TEXT NOT NULL UNIQUE`, `owner_account_id UUID REFERENCES account(id) ON DELETE SET NULL`, `description TEXT`, `status TEXT NOT NULL DEFAULT 'active'`, `delete_requested_at TIMESTAMPTZ`, `created_at`/`updated_at TIMESTAMPTZ NOT NULL DEFAULT now()`; index `map_owner_idx (owner_account_id)`. **Do not** add `last_checkpoint_seq/at` or `retention_days`.
  - `acl` — `id`, `name TEXT NOT NULL`, `owner_account_id UUID REFERENCES account(id) ON DELETE SET NULL`, `created_at`/`updated_at`. **No** `pending_delete_at`.
  - `acl_member` — `id`, `acl_id UUID NOT NULL REFERENCES acl(id) ON DELETE CASCADE`, `member_type TEXT NOT NULL`, `eve_entity_id BIGINT`, `character_id UUID REFERENCES eve_character(id) ON DELETE CASCADE`, `name TEXT NOT NULL DEFAULT ''`, `permission TEXT NOT NULL`, `created_at`/`updated_at`.
  - `map_acl` — `map_id UUID NOT NULL REFERENCES map(id) ON DELETE CASCADE`, `acl_id UUID NOT NULL REFERENCES acl(id) ON DELETE CASCADE`, `created_at`, `PRIMARY KEY (map_id, acl_id)`.
- [x] 1.2 Add the `acl_member` CHECK constraints (one migration statement or inline): `member_type IN ('character','corporation','alliance')`; `permission IN ('read','read_write','manage','admin','deny')`; `member_type = 'character' OR permission NOT IN ('manage','admin')`.

## 2. DB layer (`backend/src/db/`)

- [x] 2.1 `db/map.rs` — `Map` struct (incl. `status`, `delete_requested_at`); `insert_map`, `find_map_by_id`, `update_map` (name/slug/description; surface slug-uniqueness violation distinctly so the service can map it to a conflict), `soft_delete_map` (set `status` + `delete_requested_at = now()`), and `find_maps_for_account` (owned + resolved-grant via attached ACLs, active only). A `MapWithAcls` shape for the annotated listing.
- [x] 2.2 `db/acl.rs` — `Acl` struct (no `pending_delete_at`); `insert_acl`, `find_acl_by_id`, `update_acl_name`, `delete_acl` (hard delete; FK cascades members + attachments), `find_acls_manageable_by_account` (owner OR character member holding `manage`/`admin`), ordered by name.
- [x] 2.3 `db/acl_member.rs` — `AclMember` struct; `MemberType` and `AclPermission` enums (strum, snake_case); `add_member`, `list_members(acl_id)`, `update_member_permission`, `remove_member`.
- [x] 2.4 `db/map_acl.rs` — `attach_acl(map_id, acl_id)`, `detach_acl(map_id, acl_id)`, `find_acl_ids_for_maps(&[map_id])` (for the annotated listing).
- [x] 2.5 Unit tests (`#[sqlx::test]`, one function per test): map insert/find/update/soft-delete + slug-unique violation; acl insert/find/rename/delete-cascade; manageable-ACLs query (owner, manager, unrelated); member add/list/update/remove; the three `acl_member` CHECKs reject bad rows; attach/detach + PK-collision behavior.

## 3. Permission resolver (`backend/src/permissions.rs`)

- [x] 3.1 Add top-level module `permissions.rs` (declared in `lib.rs`/`main.rs`): the ordered `Permission` enum (`Read < ReadWrite < Manage < Admin`, derive `Ord`) with `as_str`/`FromStr`.
- [x] 3.2 `effective_permission(pool, account_id, map_id) -> Result<Option<Permission>>`: owner bypass querying `status = 'active'` (NOT `deleted = false`); else the member-match query (character/corp/alliance join against `eve_character`); `deny` ⇒ `None`; else most-permissive `max()`.
- [x] 3.3 Unit tests: permission ordering; round-trip parse; `deny` parses outside the enum; resolver returns Admin for owner of an active map; owner of a soft-deleted map gets no owner bypass; corp/alliance/character matches resolve; deny overrides; no-match ⇒ None.

## 4. Error & audit plumbing

- [x] 4.1 `error.rs` — add the conflict case(s) this surface needs (e.g. `ConflictKind::MapSlugAlreadyExists`) and confirm `AppError::Forbidden`/`NotFound` cover ACL-owner mismatch and missing rows. **No** per-service error enums (`MapError`/`AclError`) — fold into the single `AppError` per the skill.
- [x] 4.2 `audit/mod.rs` — add variants in the existing `kind`-string house style: `MapCreated`, `MapDeleted`, `AclAttachedToMap`, `AclDetachedFromMap` (and ACL/member mutation variants as needed). Unit-test the kind strings if the module tests them today.

## 5. Service layer (`backend/src/services/`)

- [x] 5.1 `services/acl.rs` — create/rename/delete ACL (owner-gated); add/list/update/remove member; `list_manageable_for_account`. Validate member shape: `member_type = 'character'` requires `character_id`; corp/alliance require `eve_entity_id` (service-layer check beyond the CHECKs). No HTTP types imported.
- [x] 5.2 `services/map.rs` — `list_maps`, `get_map`, `create_map` (optional `acl_id` attach in the same tx, owner-checked), `update_map`, `delete_map` (soft), `attach_acl_to_map`, `detach_acl_from_map`. Gate every map op through `permissions::effective_permission` at the documented minimum (read/manage/admin). Gate attach on ACL ownership. Emit audit events. **Strip every `map_event::append_event` call** from the reference — audit only; there is no `map_event` module in this change.
- [x] 5.3 Unit tests (mock db / resolver where the skill prescribes): create maps slug-conflict path; update requires manage; delete requires admin; attach requires admin + ACL ownership; resolver-gating refuses below-threshold; member-shape validation rejects a corp member with `character_id` and a character member missing `character_id`.

## 6. DTOs (`backend/src/dto/`)

- [x] 6.1 `dto/map.rs` — `MapResponse` (+ attached `AclSummary` list), `MapListResponse`, `CreateMapRequest` (name/slug/description + optional `acl_id`; validate slug regex + lengths), `UpdateMapRequest`, `AttachAclRequest`. `From<Map>`/`From<MapWithAcls>`. No `#[serde(flatten)]`, no `Serialize` on db models.
- [x] 6.2 `dto/acl.rs` — `AclResponse`, `AclListResponse`, `AclMemberResponse`, `AclMemberListResponse`, `CreateAclRequest`/`RenameAclRequest` (name length), `AddMemberRequest`, `UpdateMemberRequest`. `From<Acl>`/`From<AclMember>`.

## 7. Handlers & routing (`backend/src/handlers/api/v1/`)

- [x] 7.1 `handlers/api/v1/acls.rs` — `GET/POST /acls`, `PATCH/DELETE /acls/{acl_id}`, `GET/POST /acls/{acl_id}/members`, `PATCH/DELETE /acls/{acl_id}/members/{member_id}`. `AuthenticatedAccount` extractor; validate request bodies; one service call per op; `ApiResponse::data` envelope; map service errors to `AppError`. utoipa annotations.
- [x] 7.2 `handlers/api/v1/maps.rs` — `GET/POST /maps`, `GET/PATCH/DELETE /maps/{map_id}`, `POST /maps/{map_id}/acls`, `DELETE /maps/{map_id}/acls/{acl_id}`. Same conventions.
- [x] 7.3 Wire both route groups into the `/api/v1` router and the OpenAPI doc.
- [x] 7.4 Handler unit tests (mock service): envelope shape, validation dispatch, error→status mapping (403 forbidden, 404 not found, conflict status for slug).

## 8. Integration tests (`backend/tests/`)

- [x] 8.1 End-to-end per handler (happy + key error paths) via `#[sqlx::test]`: ACL CRUD + members; map CRUD; attach/detach; a resolver path proving a corp/alliance grant lets a non-owner read a map and a `deny` refuses it; slug conflict ⇒ conflict status; below-threshold ⇒ 403.

## 9. HURL (`backend/tests/hurl/`)

- [x] 9.1 `acls.hurl` — every ACL + member endpoint, asserting the `data` envelope shape and status codes.
- [x] 9.2 `maps.hurl` — every map + attach/detach endpoint, asserting envelope + statuses, including a forbidden and a slug-conflict case.

## 10. Verification

- [x] 10.1 `cargo fmt` and `cargo clippy --all-targets` clean (from `backend/`).
- [x] 10.2 Regenerate the sqlx offline cache: `cargo sqlx prepare -- --all-targets` (from `backend/`); commit the `.sqlx/` diff.
- [x] 10.3 `cargo sqlx prepare --check -- --all-targets` passes (no cache drift).
- [x] 10.4 `cargo test` (backend unit + integration) passes.
- [x] 10.5 Ran the HURL suite against the live dev stack: `acls.hurl` 12/12, `maps.hurl` 15/15 — `data` envelope, slug-conflict (409 `map_slug_already_exists`), forbidden/404 cases, the corp-grant resolver path, and attach/detach all confirmed live. Regression: `me.hurl` 2/2, `keys.hurl` 8/8, `preferences.hurl` 7/7, `health.hurl` 1/1 green. (`blocks.hurl` needs an `admin_session` var and `account.hurl`/`session.hurl` are destructive/cookie-jar-sensitive per the README — skipped, not regressions.)

> This change is **backend-only** — it adds no frontend code — so the `CLAUDE.md` frontend verification trio (`pnpm --filter frontend test` / `run check` / `run test:e2e`) does not apply. If any task here grows to touch `frontend/`, that trio becomes mandatory and must be added to this section.
