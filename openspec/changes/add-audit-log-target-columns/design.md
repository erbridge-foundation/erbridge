## Context

The shipped `audit-log` capability records every state-changing action with a first-class, indexed **actor** (`actor_account_id` + snapshotted `actor_character_id` / `actor_character_name`) but stuffs the **target** of the action into `details` JSONB. The audit log had no reader when that decision was made, so the asymmetry was invisible.

The next change (`add-server-admin-and-block-list`) introduces the first reader — an admin audit browser — and its dominant query is target-first: "who blocked / promoted / removed **this** character or account?" Target search today is a sequential scan over the table plus per-row JSONB extraction. This change makes the target a first-class, indexed axis so the table reflects its own actor→action→target grammar and the browser consumes a clean filter.

Constraints carried in from the shipped capability:
- `audit_log` is **INSERT-only**. The new columns are written once at insert and never updated (the backfill `UPDATE` is a one-shot migration step against historical rows, not an application path).
- `event_type` strings are **stable** and SHALL NOT change.
- Actor character columns are **snapshots** — frozen at write time, no FK, survive hard-deletes/renames. The new `target_name` for account targets follows the identical snapshot discipline.

## Goals / Non-Goals

**Goals:**
- Make target a first-class, indexed query axis on `audit_log`.
- Keep each event's "what is my target" knowledge in the `AuditEvent` enum, alongside `event_type()` / `details()`.
- Snapshot account-target names so a human searching by character name finds account-targeted events (grants, deletions) the same way they find character-targeted ones.
- Backfill existing rows so the columns are not sparse.
- Leave `list_audit_log` able to filter target-first.

**Non-Goals:**
- No admin UI / browser ships here (that is `add-server-admin-and-block-list`).
- No change to `details` shape, `event_type` strings, or actor columns.
- No `pg_trgm` / fuzzy search in this change — `target_name` search is exact-prefix/`ILIKE` against a btree-backed expression index; fuzzy substring search is a later optimisation if scale demands it.
- No new event variants and no change to which variants are emitted vs dormant.

## Decisions

### Decision 1: Populate via `AuditEvent::target()`, not Postgres GENERATED columns

`AuditEvent` gains `fn target(&self) -> Option<AuditTarget>`, and `record_in_tx` writes the three columns from it. Knowledge of "what does this event target" lives in the enum next to `event_type()` and `details()` — one match arm per variant, mechanical and reviewable in Rust.

**Alternative considered — Postgres `GENERATED ALWAYS AS ... STORED` from `details`:** zero write-path change and auto-backfills, but `target_type` and `target_name` require a `CASE` over `event_type` inside SQL — re-encoding the event vocabulary in a second language, out of sync with the enum. It also cannot produce the account-target **name** (which requires a write-time lookup, not a JSONB read). Rejected: it pushes event semantics into the schema in the wrong place and can't satisfy Decision 3.

### Decision 2: `target_id` is `TEXT`, with `target_type` discriminating

Targets are heterogeneous: characters are EVE `BIGINT` IDs, accounts/maps/ACLs are `UUID`s. Rather than three nullable typed columns (`target_account_id UUID`, `target_character_id BIGINT`, …) the design uses one `target_id TEXT` plus a `target_type` discriminator (`'character'`, `'account'`, `'map'`, `'acl'`). `AuditTarget` carries the type, so the stringification is centralised and the column count stays flat as future target kinds (map, acl) activate.

**Alternative considered — one typed column per kind:** more "correct" typing but a widening column set, mostly-NULL rows, and per-kind index proliferation. Rejected for a denormalised audit table whose target is already a snapshot, not a referential key (no FKs here by design). `TEXT` + discriminator matches the table's existing snapshot philosophy.

### Decision 3: Account-target `target_name` snapshots the main character name at write time

An account has no name of its own; the name a human searches by is its **main character's** name. For account-targeted events (`server_admin_granted`, `server_admin_revoked`, `account_deletion_requested`, `account_reactivated`, `account_purged`, `orphan_character_claimed`'s account, etc.), `record_in_tx` looks up the **target** account's main character within the same transaction and snapshots its name into `target_name` — exactly mirroring the actor-name snapshot. This reuses `characters::get_main_for_account_tx`.

The lookup is **fail-soft**, identical to the actor path: if the target account has no main (invariant violation), emit `tracing::error!` and write `target_name = NULL` (still write `target_type`/`target_id`); never abort the caller's transaction over an audit-name miss.

For **character-targeted** events the name is already in hand (the variant already carries `character_name`, or it is the same character being acted on) so no lookup is needed.

**Alternative considered — leave account `target_name` NULL:** no lookup, but account-targeted events become invisible to name search — the admin can't answer "who promoted *Bob* to admin" by typing "Bob". Rejected: defeats the target-name axis for half the admin-relevant events.

### Decision 4: Backfill existing rows in the migration

The table is days old and holds only registration + test rows. The migration runs an `UPDATE` deriving `target_type`/`target_id` from `details` for existing rows (a one-shot, INSERT-only-invariant-preserving exception scoped to migration time). `target_name` for already-existing **account-targeted** rows is best-effort: the migration can join to the account's current main where one exists; where it can't, the column stays NULL (historical, low-stakes). Going forward, `record_in_tx` populates all three at insert.

**Alternative considered — accept NULL on old rows:** simpler, but leaves the columns sparse and old rows unsearchable target-first for no real saving given the trivial row count. Rejected.

### Decision 5: Indexes — partial btree on `target_id`, expression index for name search

- `audit_log_target_id_idx ON audit_log (target_type, target_id) WHERE target_id IS NOT NULL` — the "all events against this entity" query, partial because target-less rows (e.g. a future system sweep) don't participate.
- `audit_log_target_name_idx ON audit_log (LOWER(target_name)) WHERE target_name IS NOT NULL` — backs case-insensitive name search via `LOWER(target_name) = LOWER($1)` / `LIKE LOWER($1) || '%'`. Btree on `LOWER(...)` is sufficient at current scale; `pg_trgm` for mid-string substring search is deferred (Non-Goals).

### Decision 6: `list_audit_log` gains optional `target_type` / `target_id` / `target_name` filters

Three new optional bound parameters, same parameterised pattern as the existing filters (no interpolation). `target_name` matches case-insensitively against the `LOWER()` expression index. Existing callers passing the old argument set continue to compile only if the signature is extended compatibly — since this is an internal helper with a handful of call sites, the signature is widened and all call sites updated in the same change (no external API).

## Risks / Trade-offs

- **Extra in-tx lookup for account-targeted events** → Mitigation: it's the same cheap indexed main lookup the actor path already does; account-targeted events are low-frequency (admin grants, deletions), not hot-path.
- **`target_id` as TEXT loses DB-level type checking / referential integrity** → Mitigation: audit columns are deliberately snapshot-only with no FKs (matching actor columns); integrity is the writer's responsibility, enforced by `AuditEvent::target()` being the single source and unit-tested per variant.
- **Backfill `target_name` for old account rows may be incomplete** → Mitigation: acceptable — historical rows, trivial count; `target_type`/`target_id` are still populated so those rows remain target-id-searchable.
- **Forgetting `target()` for a future variant** → Mitigation: `target()` returns `Option<AuditTarget>` with an explicit arm per variant; dormant variants get their correct target now (unit-tested), so activating a dormant variant later needs no audit-side change. A `#[deny]`-style exhaustiveness is naturally enforced by the `match` having no wildcard arm.
- **Sequencing coupling** → `add-server-admin-and-block-list` already proposed; its `audit-log` delta and audit-browser tasks must be revised to consume these columns. Mitigation: that change is not yet applied, so its artifacts can be updated before implementation; this is called out in Impact.

## Migration Plan

1. Apply `00000000000006_add_audit_log_target_columns.sql`: add `target_type`, `target_id`, `target_name`; create the two indexes; run the backfill `UPDATE`.
2. Land `AuditTarget` + `AuditEvent::target()` + `record_in_tx`/`list_audit_log` changes; regenerate `.sqlx/`.
3. Rollback: drop the three columns and two indexes. No data loss beyond the additive columns; `details` retains the source of truth, so a re-derive is always possible.
