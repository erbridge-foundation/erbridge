# Redesign Audit Filtering

## Why

The `/admin/audit` browser presents five raw free-text inputs (`target_name`, `event_type`, `target_type`, `target_id`, `actor`) over a `GET` form. Four of the five demand an **exact** string the admin must already know — `event_type = "acl_member_permission_changed"` typed from memory, or a pasted account/character UUID — and the one fuzzy field (`target_name`) is unindexed. The result is an interface you can only use if you already know the answer.

It also serves only one of the three ways an admin actually arrives at an audit log:

- **Directed** — "what did this pilot do?" / "what happened to this ACL?" — the admin holds a name, not a row. Today they must type an exact value or guess.
- **Refining** — "…and narrow to just the ACL events" — there is no way to drill in from a result; every filter is a separate typed box.
- **Browsing** — "what's been going on lately?" — the admin holds nothing and just wants to read the recent stream. Today that is the undifferentiated default list with a "Load older" button.

No time bound exists, so every query and the unindexed `target_name` substring scan range over all of history.

## What Changes

Redesign the audit browser around how a human reads a log: **search to enter, click to refine, browse by default** — all bounded by a recent time window.

- **Time window, default last 7 days.** A new tiered window control (`<7d` default, `30d`, `90d`, `365d`, then per-year buckets) is the primary dial. It bounds every query — search, browse, and refine — to a recent slab by default, and is the single knob that scales recall and cost together when widened. The deepest selectable bucket is one year, so no UI path triggers an unbounded all-history scan.
- **One search box, implicit substring, both name columns.** Replaces the five raw inputs' name role with a single box that matches `actor_character_name` OR `target_name` as a case-insensitive substring (`%fragment%`), so typing `wasp` finds `The Wasp`, `Wasp 223`, and `Red Wasp Industries` — required by EVE's multi-token naming. LIKE metacharacters are escaped (literal `%`/`_`). No user-visible wildcard syntax. Search submits on Enter (not live autocomplete).
- **Closed-set filters become `<select>`s.** `event_type` (the 31-variant `AuditEvent` catalogue, static) and `target_type` (`account`/`character`/`map`/`acl`) become dropdowns, usable from a cold start. This removes the worst usability cliff (typing cryptic snake_case from memory).
- **Click any cell to refine.** Each result cell is clickable to set the matching filter: Actor cell → `actor` (account); Event cell → `event_type`; Target cell → `target_type` + `target_id` (the durable id behind the displayed name, so clicking a since-deleted entity's snapshot name still surfaces all its history). Clicking a second value in a column **replaces** the first. Active filters render as removable chips.
- **Column headers match filter names, 1:1.** Each header names the filter its cells set.
- **`target_id` paste box retained** as the one literal-id escape hatch ("I have the id, jump straight there").
- **Browse experience.** The default (no filters, 7-day window) becomes a deliberate reading surface: rows grouped under day headers (Today / Yesterday / date), event-type styling so anomalies (blocks, hard-deletes) stand out, and infinite scroll within the window using the existing keyset cursor — stopping at the window edge with a "widen?" affordance rather than silently scanning older history.

Explicitly **not** included: autocomplete dropdowns, a derived name dictionary, the `pg_trgm` extension, or any new index. At wormhole-mapper scale (realistically ≤1M rows over years, and the default query scans a 7-day slab of hundreds–thousands of rows) a windowed substring scan is effectively free. If a deployed instance ever outgrows this, the answer is cold-storage archiving of old audit rows, not added query infrastructure — noted as the escalation path, not built here.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `audit-log`: `list_audit_log` gains a combined name-search axis (`actor_character_name` OR `target_name` substring) and a `since` lower time bound, alongside the existing `before` keyset cursor. The standalone `target_name` axis is retained (still set by the Target-cell-click path via `target_id`, and available to other callers).
- `server-administration`: `GET /api/v1/admin/audit` gains `q` (combined name search) and `window`/`since` (lower time bound) query parameters; the `/admin/audit` frontend route is redesigned around search-first entry, click-to-refine, closed-set selects, and the 7-day default browse experience.

## Impact

- Backend: `backend/src/audit/mod.rs` (`list_audit_log` — add `q` OR-search and `since` bound; existing escaping/keyset reused), `backend/src/handlers/api/v1/admin.rs` (`AuditQuery` + `list_audit` — accept `q`, `window`/`since`, map window tier → `since`), `backend/src/dto/admin.rs` (unchanged response shape; entries already carry every column the UI needs).
- Frontend: `frontend/src/routes/admin/audit/+page.svelte` (full redesign — search box, window/event/target-type selects, `target_id` box, clickable cells, chips, day grouping, infinite scroll), `frontend/src/routes/admin/audit/+page.server.ts` (forward `q`/`window`, default 7d), `frontend/src/lib/api.ts` (`AuditLogQuery` gains `q`/`window`/`since`).
- i18n: new keys for the window tiers, search box, event/target-type select labels, day-group headers, chip/clear labels, window-edge "widen" affordance — across en/de/fr (and any event_type / target_type labels surfaced in the selects).
- No migration. The existing `audit_log_occurred_at_idx` backs the window range; no new index, no extension.
- Tests: backend unit/integration for the `q` OR-search and `since` bound and their conjunction with existing filters; HURL for the new query params; frontend Vitest (cell-click → filter, replace-within-column, chip removal, window default), `pnpm --filter frontend run check`, and Playwright e2e (search → result → click-to-refine → clear; browse infinite scroll within window).
