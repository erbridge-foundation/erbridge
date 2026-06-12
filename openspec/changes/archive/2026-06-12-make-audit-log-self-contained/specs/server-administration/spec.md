## MODIFIED Requirements

### Requirement: Audit browser is a search-first, click-to-refine, browse-by-default surface

The `/admin/audit` frontend route SHALL present the audit log as a single surface serving three usage modes — **directed** (the admin knows a name/event), **refining** (narrowing an existing result), and **browsing** (grazing the recent stream) — distinguished only by filter state.

**Entry controls (usable from a cold start, before any row is rendered):**

- A **time-window** control defaulting to the last 7 days, offering the tiers `7d`/`30d`/`90d`/`365d` and per-year buckets, mapped to the endpoint's `window` parameter. It is the primary dial and bounds every query.
- A single **search box** that submits on Enter (not live autocomplete) and maps to the endpoint's `q` parameter — matching a case-insensitive substring of the actor name, the target name, **or the event's `details` payload** (so a name snapshotted only in `details`, such as an ACL member, is findable). No user-visible wildcard syntax is presented; metacharacters are handled server-side.
- An **event-type `<select>`** populated from the static `AuditEvent` catalogue, mapping to `event_type`.
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
- **THEN** results within the window whose actor name, target name, or `details` text contains "wasp" are shown, newest-first, without the admin having clicked any row

#### Scenario: Search finds a member named only in details
- **GIVEN** an `acl_member_added` row whose member name "Wasp 222" is carried in `details`
- **WHEN** the admin searches for "Wasp 222"
- **THEN** the row is returned, answering "who added Wasp 222 to which ACL" via the row's actor and target columns

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
- **THEN** all `AuditEvent` variants are listed and selecting one filters the result, without requiring any row to be present first

#### Scenario: Browse groups by day and scrolls within the window
- **GIVEN** the default view with no filters
- **THEN** rows are grouped under Today / Yesterday / dated headers
- **WHEN** the admin scrolls to the bottom of the loaded rows and more rows exist within the 7-day window
- **THEN** the next older page is fetched and appended via the keyset cursor

#### Scenario: Window edge offers widening rather than silent expansion
- **GIVEN** the admin has scrolled to the oldest row within the active window
- **WHEN** no more rows exist inside that window
- **THEN** scrolling stops and an affordance to widen the window is shown; the view does not silently fetch rows older than the window

## ADDED Requirements

### Requirement: Audit browser exposes a per-row Details view

Each audit result row SHALL offer a non-destructive **Details** affordance that opens a dialog rendering that row's `details` payload as a generic key/value list (one row per top-level field). The dialog SHALL render the snapshotted values verbatim and SHALL NOT resolve any id to a name at view time (it relies on names being snapshotted at write time). The dialog SHALL be dismissable and SHALL not mutate any state. Its chrome (title, close control, empty state) SHALL be internationalised across the project's supported locales (en/de/fr).

#### Scenario: Opening Details shows the snapshotted fields
- **GIVEN** an `acl_member_added` row whose `details` contains `member_name = "Wasp 222"`, `member_type = "character"`, and `permission = "admin"`
- **WHEN** the admin activates that row's Details affordance
- **THEN** a dialog opens listing each `details` field as a key/value pair, including `member_name: Wasp 222`, so the admin can read who was added without leaving the page

#### Scenario: Details dialog performs no resolution and no mutation
- **WHEN** the Details dialog is open for any row
- **THEN** it displays only the values already present in `details` (no live lookups), and dismissing it leaves the audit log and the result set unchanged

#### Scenario: Empty details renders gracefully
- **GIVEN** an event whose `details` payload is empty (`{}`)
- **WHEN** the admin opens its Details dialog
- **THEN** the dialog shows an empty-state message rather than an empty or broken list
