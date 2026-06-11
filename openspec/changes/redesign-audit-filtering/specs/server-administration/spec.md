## MODIFIED Requirements

### Requirement: GET /api/v1/admin/audit exposes the audit log to admins

`GET /api/v1/admin/audit` SHALL return audit-log entries newest-first via `audit::list_audit_log` (per the `audit-log` capability). It SHALL accept these optional query parameters:

- `event_type`, `actor` (account UUID), `target_type`, `target_id` — equality filters.
- `target_name` — case-insensitive substring match.
- `q` — a combined name search matching **either** the actor character name **or** the target name as a case-insensitive substring (e.g. `wasp` finds rows where Wasp was the actor *or* the target, including `The Wasp` and `Red Wasp Industries`). This is the axis a human searches on; it does not require the full name and is not anchored to the start of the name.
- `window` — a tiered relative time bound that the endpoint maps to a day-snapped `since` lower bound passed to `list_audit_log`. The accepted tiers SHALL be `7d` (the default when `window` is omitted), `30d`, `90d`, `365d`, and per-year buckets back from the current year. The deepest selectable tier SHALL be a single year, so no request issues an unbounded all-history scan. An explicit `since` (RFC 3339) MAY be accepted as an alternative to `window`; when both are absent the default 7-day bound applies.
- `before` — an RFC 3339 keyset cursor / exclusive upper time bound for pagination within the window.
- `limit` — clamped to a sensible maximum (defaulting when omitted).

All supplied filters combine conjunctively. The default 7-day window keeps the common query bounded to a recent slab using the existing `audit_log_occurred_at_idx`; no new index or Postgres extension is introduced. The response SHALL include the entries and a `next_before` cursor (the oldest returned entry's `occurred_at`) for pagination. Each entry exposes `id`, `occurred_at`, `actor_account_id`, `actor_character_id`, `actor_character_name`, `event_type`, `details`, `target_type`, `target_id`, and `target_name`.

#### Scenario: Admin reads the audit log with the default window
- **WHEN** a server admin calls `GET /api/v1/admin/audit` with no parameters
- **THEN** the response is `200` with `data.entries` newest-first within the last 7 days (capped at the clamped limit) and a `next_before` cursor; each entry carries its `target_type`/`target_id`/`target_name`

#### Scenario: Admin widens the window
- **WHEN** a server admin calls `GET /api/v1/admin/audit?window=90d`
- **THEN** entries from the last 90 days are returned, newest-first; the lower bound is day-snapped

#### Scenario: Admin searches by name across actor and target
- **WHEN** a server admin calls `GET /api/v1/admin/audit?q=wasp`
- **THEN** only entries within the active window whose actor character name OR target name contains "wasp" case-insensitively are returned, newest-first — answering "show me anything involving Wasp" regardless of which side Wasp was on, without requiring the full name

#### Scenario: Admin filters and paginates within the window
- **WHEN** a server admin calls `GET /api/v1/admin/audit?event_type=eve_character_blocked&before=<ts>&limit=20`
- **THEN** only `eve_character_blocked` entries older than `<ts>` and within the active window are returned, at most 20, newest-first

#### Scenario: Admin filters target-first by exact entity
- **WHEN** a server admin calls `GET /api/v1/admin/audit?target_type=acl&target_id=<uuid>`
- **THEN** only entries targeting that exact ACL are returned, newest-first — including a since-deleted ACL whose name survives only as a snapshot in older rows

## ADDED Requirements

### Requirement: Audit browser is a search-first, click-to-refine, browse-by-default surface

The `/admin/audit` frontend route SHALL present the audit log as a single surface serving three usage modes — **directed** (the admin knows a name/event), **refining** (narrowing an existing result), and **browsing** (grazing the recent stream) — distinguished only by filter state.

**Entry controls (usable from a cold start, before any row is rendered):**

- A **time-window** control defaulting to the last 7 days, offering the tiers `7d`/`30d`/`90d`/`365d` and per-year buckets, mapped to the endpoint's `window` parameter. It is the primary dial and bounds every query.
- A single **search box** that submits on Enter (not live autocomplete) and maps to the endpoint's `q` parameter — matching a case-insensitive substring of either the actor name or the target name. No user-visible wildcard syntax is presented; metacharacters are handled server-side.
- An **event-type `<select>`** populated from the static `AuditEvent` catalogue (the 31 variants), mapping to `event_type`.
- A **target-type `<select>`** offering `account`/`character`/`map`/`acl`, mapping to `target_type`.
- A **`target_id` text box** as the literal-id escape hatch ("I have the id, take me there"), mapping to `target_id`.

**Refinement (after results exist):**

- Each result cell SHALL be clickable to set the matching filter: an Actor cell sets `actor` to that row's account; an Event cell sets `event_type`; a Target cell sets `target_type` + `target_id` (the durable id behind the displayed snapshot name). Clicking a since-deleted entity's snapshot name SHALL filter by its id and surface all of its history.
- Clicking a second value within the same column SHALL **replace** the prior value for that column (each axis stays a single value).
- Cells with no value (system-actor rows, target-less events) SHALL be non-interactive.
- Active filters SHALL render as removable chips; an Actor chip SHALL make clear it filters by account (so sibling-character rows in the result are not surprising). A control SHALL clear all filters, returning to the default browse view.

**Column / filter correspondence:** each result column header SHALL name the filter its cells set (When, Actor, Event, Target). `target_name` is display-only — the label shown in the Target cell — and is not exposed as a separate name-equality input in this UI.

**Browse experience (default view: no filters, 7-day window):**

- Result rows SHALL be grouped under day headers (Today / Yesterday / date) so the stream reads as a timeline.
- Security-relevant event types (e.g. `blocked_login_rejected`, `*_hard_deleted`, `server_admin_*`) SHALL be visually distinguished so anomalies stand out.
- The list SHALL load additional older pages by infinite scroll within the active window, using the endpoint's `next_before` keyset cursor. On reaching the oldest row in the window, scrolling SHALL stop with an explicit affordance to widen the window; the window SHALL NOT be silently auto-widened.

This route remains within the gated `/admin` group (server-admin-only, 404 to others) per the admin-gating requirement. It is read-only per the `audit-log` INSERT-only invariant — no edit or delete affordance.

#### Scenario: Cold-start directed search
- **GIVEN** an admin who has just opened `/admin/audit` (default 7-day browse view)
- **WHEN** the admin types `wasp` into the search box and presses Enter
- **THEN** results within the window whose actor or target name contains "wasp" are shown, newest-first, without the admin having clicked any row

#### Scenario: Refine by clicking a cell
- **GIVEN** a result set is displayed
- **WHEN** the admin clicks the Target cell of a row whose target is an ACL
- **THEN** an active filter for that ACL (`target_type=acl`, `target_id=<uuid>`) is applied, a removable chip appears, and the list narrows to that ACL's events

#### Scenario: Replace within a column
- **GIVEN** an active Actor filter set by clicking "Wasp 223"
- **WHEN** the admin clicks a different row's Actor cell ("Other Pilot")
- **THEN** the Actor filter is replaced by "Other Pilot" (not combined), and the result reflects only the new actor

#### Scenario: Closed-set selects are usable from a cold start
- **WHEN** the admin opens the event-type select on a freshly loaded page
- **THEN** all 31 `AuditEvent` variants are listed and selecting one filters the result, without requiring any row to be present first

#### Scenario: Browse groups by day and scrolls within the window
- **GIVEN** the default view with no filters
- **THEN** rows are grouped under Today / Yesterday / dated headers
- **WHEN** the admin scrolls to the bottom of the loaded rows and more rows exist within the 7-day window
- **THEN** the next older page is fetched and appended via the keyset cursor

#### Scenario: Window edge offers widening rather than silent expansion
- **GIVEN** the admin has scrolled to the oldest row within the active window
- **WHEN** no more rows exist inside that window
- **THEN** scrolling stops and an affordance to widen the window is shown; the view does not silently fetch rows older than the window
