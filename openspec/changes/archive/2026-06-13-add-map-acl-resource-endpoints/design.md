# Design — add-map-acl-resource-endpoints

## Context

The maps/ACLs UI shipped against list-only read endpoints; `acls/[id]/+page.server.ts` and `maps/[slug]/settings/+page.server.ts` both fetch the full list and `find()` (each with a comment noting the missing endpoint). The default-ACL convenience in `maps/+page.server.ts` performs three sequential API calls with a best-effort middle step and no rollback story — the backend already supports `acl_id` on map create (attach in the same tx), so the missing piece is creating the ACL itself server-side.

## Goals / Non-Goals

**Goals:**
- O(1) reads for the detail/settings pages with visibility identical to the lists they replace.
- Default-ACL map creation is atomic: either map + ACL + member + attachment all exist, or none do.

**Non-Goals:**
- A generic `GET /api/v1/maps/{map_id}` by UUID already exists; it stays. By-slug is additive for the frontend's route key.
- Pagination or filtering on the new reads.
- Changing list-endpoint shapes.

## Decisions

**Visibility mirrors the lists.** `GET /acls/{acl_id}` returns 404 (not 403) when the ACL exists but the caller cannot manage it — exactly the manageable-list predicate (owner, or manage/admin via character member), so the detail page cannot leak names the list would hide. Same for by-slug: the read-permission predicate from `GET /maps/{map_id}` applies; unknown slug and unreadable map are both 404.

**By-slug is a separate route, not a query param.** `/maps/by-slug/{slug}` keeps axum routing static and the OpenAPI contract obvious. Slug charset (`[a-z0-9-]`) cannot collide with the literal `by-slug` segment under `/maps/{map_id}` because `{map_id}` parses as UUID — but a distinct prefix avoids relying on parse-order subtleties entirely.

**`default_acl` is a boolean on the existing create, not a new endpoint.** The request shape `{ name, slug, description?, acl_id?, default_acl? }` rejects `acl_id` + `default_acl` together (400) — one attach source per create. Service flow inside one tx: insert ACL (name = map name) → look up caller's main → insert `admin` character member when a main exists (no member otherwise; owner retains implicit admin via the resolver — same semantics the frontend's best-effort step had) → insert map → attach → audit all four events. The frontend's three-call orchestration and its leak disappear.

**ACL name collision is a non-issue.** `acl.name` has no uniqueness constraint; two maps named "Home" yield two ACLs named "Home", as the current frontend flow already does.

## Risks / Trade-offs

- [404-vs-403 on the ACL read] Hides existence from non-managers; consistent with the list and the frontend's current `find()` behaviour. A future "viewer" role would revisit.
- [Duplicate audit volume] Default-ACL create writes four audit rows in one tx — intentional; each is an existing event type with its own target.
- [Frontend keeps a fallback?] No — the loads switch outright; the old list-then-find path is deleted to avoid two code paths drifting.

## Migration Plan

Backend first or together with frontend in one deploy (frontend change depends on the new endpoints; the compose stack ships both at once). No schema migration. Rollback is a revert of both.
