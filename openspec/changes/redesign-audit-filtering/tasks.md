# Tasks — Redesign Audit Filtering

> Backend work follows the `rust-rest-api` skill (handler → service/audit → db layering, DTOs, response envelope, full test coverage). Frontend work follows the `sveltekit-node` skill (Svelte 5 runes, native CSS, load functions / form actions, design tokens).

## 1. Backend — `list_audit_log` gains `q` search and `since` window

- [x] 1.1 Add `q: Option<&str>` and `since: Option<DateTime<Utc>>` parameters to `audit::list_audit_log` in `backend/src/audit/mod.rs`.
- [x] 1.2 Build the `q` pattern reusing the existing LIKE-metacharacter escaping (`\`, `%`, `_`) and `%…%` wrapping already used for `target_name`; bind it for the OR clause `(actor_character_name ILIKE $q OR target_name ILIKE $q)`.
- [x] 1.3 Add the `since` lower bound (`$since::TIMESTAMPTZ IS NULL OR occurred_at >= $since`) to the WHERE clause, alongside the existing `before` upper bound; keep all axes conjunctive and parameter-bound.
- [x] 1.4 Confirm the query plan uses `audit_log_occurred_at_idx` for the `since` range (no new index, no extension).

## 2. Backend — unit/integration tests for `list_audit_log`

- [x] 2.1 `q` matches a fragment of the actor name (target unrelated) and of the target name (actor unrelated) — both rows returned.
- [x] 2.2 `q` is an unanchored, case-insensitive substring (`wasp` finds `The Wasp`).
- [x] 2.3 `q` LIKE metacharacters are literal (`%` matches a literal `%`, not a wildcard).
- [x] 2.4 `since` bounds the lower edge; `since` + `before` together return only rows in `[since, before)`.
- [x] 2.5 `q` combines conjunctively with `event_type` and `since`.
- [x] 2.6 Existing `target_name`, `target_id`, `before`, ordering, and limit scenarios still pass unchanged.

## 3. Backend — `GET /api/v1/admin/audit` handler

- [x] 3.1 Extend `AuditQuery` in `backend/src/handlers/api/v1/admin.rs` with `q: Option<String>`, `window: Option<String>`, and optional `since: Option<DateTime<Utc>>`.
- [x] 3.2 Map the `window` tier (`7d`/`30d`/`90d`/`365d`/per-year, default `7d`) to a **day-snapped** `since` (`date_trunc('day', now())` minus the interval; per-year buckets snap to year boundaries). Cap the deepest selectable tier at one year. Prefer an explicit `since` if provided.
- [x] 3.3 Pass `q` and the resolved `since` through to `svc::list_audit_log`; keep `before`/`limit` clamping behaviour.
- [x] 3.4 Update the `#[utoipa::path]` params block (add `q`, `window`, `since`).
- [x] 3.5 Response shape (`AuditLogPageDto`) and `next_before` cursor unchanged — verify no DTO change is needed.

## 4. Backend — HURL coverage

- [x] 4.1 Add cases to the admin audit HURL suite: default window returns recent entries; `window=90d` widens; `q=<fragment>` matches actor- and target-side; `target_type`+`target_id` exact entity; conjunction of `q`+`event_type`+`window`. Run live against a seeded DB.

## 5. Frontend — API client

- [x] 5.1 Extend `AuditLogQuery` in `frontend/src/lib/api.ts` with `q?`, `window?`, and `since?`; forward them as query params in `listAuditLog`.

## 6. Frontend — `/admin/audit` load function

- [x] 6.1 In `frontend/src/routes/admin/audit/+page.server.ts`, read `q` and `window` (default `window=7d` when absent) plus the existing axes and `before`; forward to `listAuditLog`.
- [x] 6.2 Return the active filter state (including `q` and `window`) so the page can render chips/selects and build the "load older" / widen links.

## 7. Frontend — `/admin/audit` page redesign

- [x] 7.1 Replace the five raw inputs with: the window `<select>` (7d default + tiers), the single search box (Enter-to-search, maps to `q`), the event-type `<select>` (static 31-variant catalogue), the target-type `<select>`, and the `target_id` text box.
- [x] 7.2 Make result cells clickable to set filters — Actor → `actor` (account), Event → `event_type`, Target → `target_type`+`target_id`; replace-within-column; non-interactive for null cells.
- [x] 7.3 Render active filters as removable chips (Actor chip worded as "account of …"); add a Clear-all control. Ensure column headers name their filters (When/Actor/Event/Target).
- [x] 7.4 Browse experience: group rows under day headers (Today/Yesterday/date); style security-relevant event types distinctly.
- [x] 7.5 Infinite scroll within the window via the `next_before` cursor (intersection-observer → fetch next page → append); at the window edge, stop and show a "widen window" affordance rather than auto-expanding.
- [x] 7.6 Native CSS + design tokens only; no new dependencies.

## 8. Frontend — i18n

- [x] 8.1 Add keys for: window-tier labels, search placeholder/aria, event-type & target-type select labels (and option labels if surfaced), `target_id` box label, chip labels + clear-all, day-group headers (today/yesterday), and the window-edge "widen" affordance.
- [x] 8.2 Provide en/de/fr translations for every new key; keep the four locale sources in sync. Run paraglide compile from `frontend/` (not `--filter`).

## 9. Frontend — tests

- [x] 9.1 Vitest: load fn defaults `window=7d`; cell-click sets the right filter; replace-within-column; chip removal; `q` round-trips to the query.
- [x] 9.2 Playwright e2e: directed search → result → click-to-refine → clear; browse default view groups by day and infinite-scrolls within the window; widen affordance at the edge.

## 10. Verification (all must pass before the change is marked complete and before any commit lands)

- [x] 10.1 Backend: `cargo test` (unit + integration) and `cargo clippy` clean.
- [x] 10.2 Backend: live HURL admin-audit suite passes against a seeded DB.
- [x] 10.3 Frontend: `pnpm --filter frontend test` — Vitest unit/component tests.
- [x] 10.4 Frontend: `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile).
- [x] 10.5 Frontend: `pnpm --filter frontend run test:e2e` — Playwright e2e tests.
