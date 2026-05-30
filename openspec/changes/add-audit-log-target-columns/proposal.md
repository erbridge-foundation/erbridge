## Why

The `audit_log` table makes the **actor** first-class (`actor_account_id`, `actor_character_id`, `actor_character_name` are real, indexed columns) but leaves the **target** of each action buried inside the `details` JSONB. That was a defensible call when the audit log had no reader. It no longer is: the admin audit browser landing in `add-server-admin-and-block-list` is overwhelmingly **target-first** — "who blocked / promoted / removed **this** character or account?" — not actor-first ("what did Z do?"). We indexed the rarer axis. A target search today means a sequential scan plus JSONB extraction across the whole table.

Promoting the target to first-class indexed columns now — before the admin browser is built on top of it — means the browser consumes a clean, indexed filter axis instead of working around its absence.

## What Changes

- Add three columns to `audit_log`: `target_type TEXT`, `target_id TEXT`, `target_name TEXT` (all nullable — not every event has a target).
- Add a `target()` method to the `AuditEvent` enum that returns the event's target as a small typed value (`AuditTarget`). This keeps target knowledge in the enum alongside `event_type()` and `details()`.
- `record_in_tx` writes the three target columns from `event.target()`. For **account-targeted** events it snapshots the target account's **main character name** at write time (mirroring how the actor's character name is snapshotted), reusing the same in-tx main lookup.
- Add indexes for the dominant target queries: a partial index on `target_id` and a `target_name` index supporting case-insensitive name search (the axis a human actually searches on).
- Backfill the handful of existing rows' target columns in the migration from their `details` JSONB so the new columns are not sparse on historical rows.
- Extend `list_audit_log` with optional `target_type` / `target_id` / `target_name` filter axes so the future admin browser can query target-first.
- **Not breaking**: columns are additive and nullable; `details` is unchanged; `event_type` strings are unchanged; existing callers of `record_in_tx` and `list_audit_log` keep working (new `list_audit_log` filters are optional).

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `audit-log`: The `audit_log table schema` requirement gains the three target columns and their indexes. The `AuditEvent enum` requirement gains a `target()` method and a normative target catalogue (which variant targets what). The `record_in_tx` requirement gains target-column population including the account-target main-name snapshot. The `list_audit_log` requirement gains the target filter axes.

## Impact

- **Migration**: new `backend/migrations/00000000000006_*.sql` adding columns, indexes, and a backfill `UPDATE`.
- **Code**: `backend/src/audit/mod.rs` (`AuditEvent::target()`, `AuditTarget`, `record_in_tx`, `list_audit_log`, `AuditLogEntry`), and the `.sqlx/` offline query cache regenerated.
- **Sequencing**: lands **before** `add-server-admin-and-block-list`. That change's `audit-log` delta and its admin audit-browser spec/tasks will be updated to consume `target_type` / `target_id` / `target_name` filters rather than re-deriving target search from JSONB.
- **No frontend impact** in this change (no UI ships here; the browser ships with the admin change).
