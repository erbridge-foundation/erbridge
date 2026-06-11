# Design — Redesign Audit Filtering

## The three arrival modes (the design's organising idea)

An admin reaches the audit log in one of three states of mind. The interface must serve all three on the **same surface**, distinguished only by filter state:

| Mode | In the admin's head | Entry gesture |
|------|---------------------|---------------|
| **Directed** | a name / event ("did Wasp do anything?") | type in the search box, or pick an event/type select |
| **Refining** | already in a result set ("…just the ACL events") | click a cell to narrow |
| **Browsing** | nothing — grazing the recent stream | land on the default 7-day view and scroll |

The earlier instinct to make click-to-refine the *primary* entry was wrong: you cannot refine your way to a starting point — refinement needs a row to already match, which is circular. **Typed/selected entry is the front door; click-to-refine is the hallway after you are inside; browse is the resting state.** The test each control must pass: *can the admin express this intent before a single row is rendered?* Search box, window, event select, target-type select, and the `target_id` box all pass (cold-start usable); Actor and Target filters are click-only by nature (their values are unbounded and only knowable from rows).

## Why the time window is the keystone

A window (default last 7 days) bounds the working set for **every** mode at the source. The expensive thing was never "the log is big" — it was letting every query see all of history. With a default 7-day bound, the common query scans a recent slab (hundreds–low-thousands of rows at this app's scale) using the existing `audit_log_occurred_at_idx` range. Widening the window is the single knob that expands recall and cost together, deliberately, on the rare historical hunt.

Tiered buckets (`<7d`, `30d`, `90d`, `365d`, then per-year) rather than a free date picker because:

- They match how a human frames an audit hunt ("the last week", "this quarter", "back in 2024") — not absolute timestamps.
- A handful of discrete boundaries are planner- and cache-friendlier than infinite distinct `since` values.
- The deepest selectable bucket is **one year**, so no UI path ever issues an all-history scan. The unbounded-scan worst case becomes structurally unreachable, not merely discouraged.
- A *relative* default ("last 7 days") is correct on every page load forever, where an absolute default goes stale.

Boundaries SHALL be **day-snapped** (`date_trunc('day', now()) - interval`) so the predicate is stable within a day — the window does not visibly drift mid-session, and the query is cacheable.

## Why no autocomplete, dictionary, pg_trgm, or new index

This was explored at length; the conclusion is to design the scaling problem out of existence rather than build infrastructure for it:

- **Scale reality.** This is an EVE wormhole mapper. A realistic large/old instance is ≤~1M audit rows over years, not 10M+. Within the default 7-day window the search scans a few thousand rows. A `%fragment%` substring scan over that is effectively instant — index or not.
- **Autocomplete ≠ search.** A human does not need a live dropdown materialising as they type; they need to type `wasp`, press Enter, and get results. Dropping live autocomplete keeps the humane behaviour (search) and removes the plumbing.
- **Click-to-refine replaces value discovery.** You never need to *discover* what values exist, because the result rows already show real values — you click one. Every click-set filter is an **indexed equality** (`event_type =`, `actor_account_id =`, `(target_type, target_id) =`), the cheapest query there is.
- **Substring, not anchored prefix.** EVE names are multi-token with the identifying word often not first (`The Wasp`, `Red Wasp Industries`). Anchored `wasp%` would silently miss those — a baffling failure. The use case demands `%wasp%`; the scale permits it. (Anchored prefix would let the `LOWER(target_name)` btree range-scan, but that optimisation solves a cost problem this app does not have.)
- **Escalation, documented not built.** If a deployed instance ever feels slow on a widened search, the answer is **cold-storage archiving** of old audit rows (keeping the live table small) — consistent with the table's append-only, never-mutated invariant. Not `pg_trgm`, not a derived dictionary, not a GIN index on the hot append-only write path.

## The single search box: combined actor-OR-target name

One box matching `actor_character_name` OR `target_name` (case-insensitive substring), because when an admin types `wasp` they usually do not yet care whether Wasp was the actor or the target — they want "Wasp, anywhere." The box is deliberately **loose** (substring, both columns, OR) precisely because click-to-refine is **precise** (exact, one column, one entity): loose entry, tight refine. A returned row that matched as actor and another that matched as target sit together; the admin scans, sees the one they meant, and clicks its cell to pin the role.

LIKE metacharacters (`%`, `_`, `\`) in the input are escaped so a literal `50%` matches the characters, not wildcards — reusing the escaping `list_audit_log` already applies to `target_name`.

## Click-to-refine semantics

- **Actor cell** → filter `actor` = the row's `actor_account_id`. The cell shows a character *name* but filters by *account*, so results may include other characters of that account ("what did this person do"). The chip SHALL make this clear (e.g. "Account of Wasp 223") so sibling-character rows are not surprising.
- **Event cell** → filter `event_type` (also reachable via the select).
- **Target cell** → filter `target_type` + `target_id` (the durable id behind the displayed name). Because the row carries a *snapshot* name, clicking a since-deleted entity's name still filters by its id and surfaces all of its history — this is how the "surface historical/deleted entities" need is met with zero autocomplete or dictionary.
- **Replace within a column.** Clicking a second value in a column swaps the filter (keeps each axis a single equality). OR-within-a-column is deferred unless asked for.
- **Null cells** (system actor, target-less events) are non-interactive — the affordance must not lie.
- `target_id` paste box is retained as the literal-id front door for "I already have the id."

## Columns match filters, 1:1

For clickable cells to be legible, each column header names the filter its cells set. `target_name` is **display-only** — it is the label shown in the Target cell, not its own filter axis in the UI (the cell filters by `target_id`). The standalone `target_name` substring axis remains in `list_audit_log` for the `q` search and other callers, but the redesigned UI does not expose a separate name-equality box.

## Browse mode

The default view (no filters, 7-day window) is a first-class reading surface, not "the absence of a query":

- **Day grouping.** Rows under Today / Yesterday / `<date>` headers so the eye orients by time — pure frontend grouping over the same keyset query.
- **Event styling.** Security-relevant events (`blocked_login_rejected`, `*_hard_deleted`, `server_admin_*`) are visually distinct so anomalies stand out while grazing.
- **Infinite scroll within the window.** Fetch the next keyset page (existing `next_before` cursor) as the admin nears the bottom, appending rows — no "Load older" click interrupting the graze.
- **Window-edge boundary.** At the oldest row in the window, scrolling stops with an explicit "End of last 7 days — widen to 30 days?" affordance. The window is **not** silently auto-widened on scroll: that would defeat the cost bound and surprise the admin with a slow query they did not ask for. Widening is a deliberate act.

## Backend shape

`list_audit_log` already has the substring `target_name` axis, the `before` keyset cursor, parameter binding, and metacharacter escaping. This change adds two axes:

- `q: Option<&str>` — combined `(actor_character_name ILIKE $q OR target_name ILIKE $q)`, wrapped `%…%`, escaped. Conjunctive with all other axes.
- `since: Option<DateTime<Utc>>` — lower bound `occurred_at >= since`, complementing the existing `before` upper bound; together they express the window.

The handler maps the tiered `window` query param to a day-snapped `since`. The DTO/response shape is unchanged — entries already carry every column the redesigned UI renders and clicks on.

## Open questions deferred (not blocking this change)

- OR-within-a-column refinement (multi-actor, multi-event) — start with replace; add only if a real workflow needs it.
- A "since I last visited" personal high-water mark for browse — a fixed window slab is enough for v1.
- A When-cell click gesture ("around this time ±1h") — speculative; the window select covers time for now.
