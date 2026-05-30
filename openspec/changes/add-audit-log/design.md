## Context

The backend today has no durable record of who performed any state-changing action. Sensitive operations — account soft-delete, character promotion/removal, API key create/revoke, and (in the upcoming `add-server-admin-and-block-list` change) server-admin grants and block-list edits — leave only `tracing` logs, which are ephemeral, not queryable, and not tied to the actor.

A prior iteration of this codebase (preserved at `zz-ref/backend/older-iteration/src/audit.rs`, `tests/audit_log.rs`, and migration `0004_create_audit_log.sql`) shipped a mature audit design that we are porting with three deliberate refinements:

1. **Actor-character snapshot columns** are added to the schema. The older iteration stored only `actor_account_id`. When that account row goes away (today: never; future: when hard-delete-after-grace ships), the audit row's actor pointer becomes NULL via `ON DELETE SET NULL` and the human-readable thread is lost. Snapshotting `actor_character_id` (EVE BIGINT) and `actor_character_name` (TEXT) at write time preserves "who, in EVE terms" forever.
2. **Renames.** `ghost_character_claimed` → `orphan_character_claimed` to match current codebase terminology (`eve_character.account_id IS NULL` rows are "orphans" throughout `db/accounts.rs` and the existing specs).
3. **IP/UA capture is omitted.** The older iteration didn't capture it either; my own initial sketch added it; on reflection the threat model (small EVE community tool) does not justify the cost of correct `X-Forwarded-For` parsing behind Traefik, and the columns can be added later without retrofit pain (both nullable, additive migration, old rows simply have NULL).

The audit code lives in `backend/src/audit/mod.rs` per the `rust-rest-api` skill's module layout. The write path is transactional and explicit (called from each mutating service); there is no router-level middleware that "audits everything," which would couple audit to HTTP shape rather than to domain action.

## Goals / Non-Goals

**Goals:**

- A durable, append-only `audit_log` of state-changing actions, attributable to either an account (with their main character snapshotted) or an SSO-time signing-in character (when no session yet exists), or to the system (when neither applies).
- A Rust-side `AuditEvent` enum that acts as the catalogue of recordable actions. All ~28 variants from the older iteration are present from day one, including dormant ones (`account_purged`, map/ACL events, admin-override events, block-list events, `server_admin_revoked`, the `admin_grant` source) so the catalogue stabilises before features that use it land.
- Audit writes participate in the caller's transaction. The audit row commits with the state change, or neither does. No background jobs, no fire-and-forget.
- A read API (`list_audit_log`) that supports the three filter axes the older iteration shipped: `event_type`, `actor_account_id`, `before` (keyset cursor on `occurred_at`). Used by the admin browser in the next change; here it lives as a callable function with unit tests but no HTTP endpoint.
- Existing mutating endpoints (SSO callback, account-management writes, api-key create/revoke) are retrofitted to emit audit events. The audit table is useful from the moment the change lands, not "after the next feature."
- The actor-character columns are non-NULL for every committed row with a non-NULL `actor_account_id`, except in a single explicitly-handled fail-soft case (main lookup unexpectedly returns nothing → row writes with NULL character columns and `tracing::error!` fires).

**Non-Goals:**

- Admin-facing read endpoint or UI. Lives in `add-server-admin-and-block-list`. This change is backend-only.
- Activating dormant variants. `eve_character_blocked` / `unblocked`, `server_admin_revoked`, `server_admin_granted{admin_grant}`, map/ACL events, admin overrides, and `account_purged` are present in the enum but not emitted by any v1 code path. They light up with the feature that needs them.
- IP / User-Agent capture. Deferred as noted above.
- Cryptographic chain-of-custody (signed entries, append-only log shipping, external SIEM integration). Not in the threat model.
- Retention or rotation policy. Audit rows accumulate forever; revisit if volume warrants. For a small community tool with a handful of mutating actions per day, this won't be a problem for years.
- Auditing of *read* operations. Audit logs are for state-changing events. A separate "access log" feature would be its own change.

## Decisions

### Decision: One `audit_log` table with JSONB details, not a per-event-type schema

The older iteration's schema (ported here) is:

```sql
audit_log (
    id, occurred_at, actor_account_id, event_type TEXT, details JSONB
)
```

Plus the two new actor-character columns. Per-event payloads live in `details` as JSON.

**Why JSONB over per-event tables or per-target FK columns:**

- Per-event tables (`audit_account_registered`, `audit_character_added`, …) would explode the schema (~28 tables in v1, more later) and make "show me the chronological feed" require a UNION across all of them.
- Per-target FK columns (`target_account_id`, `target_eve_character_id`, `target_api_key_id`, …) were on the table briefly during exploration. They'd give DB-level integrity and easy joins, but they grow linearly with target types and force a `CHECK ((not null count) = 1)` constraint. For the ~3–5 distinct target types we'll ever have, the simpler JSONB approach is fine.
- JSONB lets the Rust enum define the catalogue: each variant carries the typed data, the `details()` method shapes the JSON, and the database stores an opaque blob it never has to interpret.

The Rust enum is the schema. The DB is the substrate.

### Decision: Actor identity is split across two columns — account FK and character snapshot — with three resolution paths

The schema gains:

```sql
actor_account_id     UUID    REFERENCES account(id) ON DELETE SET NULL,
actor_character_id   BIGINT,  -- EVE ID, snapshot at write
actor_character_name TEXT,    -- snapshot at write
```

The write API:

```rust
pub async fn record_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    actor_account_id: Option<Uuid>,
    acting_as: Option<ActingCharacter>,  // { eve_character_id: i64, name: String }
    event: AuditEvent,
) -> Result<()>
```

Resolution inside `record_in_tx`:

1. If `actor_account_id.is_some()` — look up the main character (one extra `SELECT` inside the tx) and write its `eve_character_id` + `name` into the two snapshot columns. This is the common case for any handler that ran through `AuthenticatedAccount`.
2. Else if `acting_as.is_some()` — write those values directly. This is the SSO-callback case where no session exists yet but we know which EVE character is signing in.
3. Else — write NULL into both character columns. This is for pure-system events (`account_purged` sweep, when it lands).

**Why the lookup lives inside `record_in_tx` rather than in an extractor:**

The application is low-volume; mutating actions are rare relative to read traffic. Doing a small extra `SELECT` inside the audit write keeps every call site trivial:

```rust
audit::record_in_tx(&mut tx, Some(account_id), None, event).await?;
```

…instead of forcing every mutating handler to extract `AuthenticatedAccountWithMain` or thread an `AuditActor` value down through service calls. The cost (one indexed `SELECT` per audit write on a low-volume system) is invisible; the call-site savings are large.

**Why two character columns instead of one:**

Snapshot semantics. `actor_character_name` exists so that when a character renames in EVE, the historical audit entry still shows the name it had at the time. Same logic for the EVE ID — though that's already stable, having it alongside the name eliminates a join to a row that may not exist anymore.

**Why no FK on the character columns:**

These are snapshots, not pointers. The whole point is that they survive a hard-delete of the referenced row. Adding `REFERENCES eve_character(eve_character_id) ON DELETE SET NULL` would defeat the purpose; `NO ACTION` would block deletes we want to allow.

### Decision: Actor-character lookup fails soft with `tracing::error!`, not loud with a returned error

When `actor_account_id` is `Some` but the main-character lookup returns no row, two options:

- **Loud**: return an error from `record_in_tx`, roll back the whole transaction including the state change that was about to commit.
- **Soft**: write the audit row with NULL character columns, fire `tracing::error!`, let the state change commit.

Picked **soft**. Reasoning:

- The invariant ("every account with characters has exactly one main") is enforced by the SSO-callback flow, not by the DB schema. The schema has `eve_character_one_main_per_account` (UNIQUE partial, prevents two mains) but no constraint that an account *must* have a main. So a missing main is, strictly, possible — for example, if the audit emit was misordered inside the SSO callback (audit fires *before* the `promote_if_no_main` step in the same tx).
- The user-facing action (registration, character add, etc.) is more important to commit than the audit's enrichment columns. Losing one audit row's character snapshot is recoverable noise; losing the user's action because the audit code couldn't enrich its row is not.
- `tracing::error!` makes the bug loud at runtime. A correctness test verifies that real SSO-callback flows produce non-NULL character columns; if that test fails, the ordering bug surfaces in CI.

The fail-soft behaviour is a documented spec scenario, not an accident.

### Decision: All ~28 enum variants present from day one, only a subset emitted in v1

The older iteration's vocabulary covers account, character, map, ACL, API key, admin (grant/revoke), admin override (hard-delete map/ACL, change ownership), and block-list events. Most of those features don't exist in this codebase yet.

The variants stay in the enum anyway. Why:

- The enum is the *catalogue of recordable actions*. Stabilising it early means downstream tooling (admin browser UI, exports, future dashboards) can know about every event type before the features that emit them land.
- Adding a variant later is non-breaking *only* if no one downstream depends on the enum's discriminants or the `event_type()` string. Front-loading the catalogue avoids ever finding out the hard way.
- The unit tests for `event_type()` and `details()` shape stand independently of integration tests. They're cheap to write once and never need to change.
- The cost is small: each variant adds ~5–15 lines of Rust (variant data + match arm in `event_type()` + match arm in `details()` + one unit test). The audit module ends up at ~700 lines, mostly machine-mechanical.

Variants present-but-silent in v1:

- `account_purged` — future hard-delete-after-grace sweep.
- `server_admin_granted{admin_grant}` source — the bootstrap source emits in v1; the admin-grant source emits in the next change.
- `server_admin_revoked` — next change.
- `eve_character_blocked`, `eve_character_unblocked` — next change.
- `map_*`, `acl_*` (created, deleted, renamed, attached/detached, member +/×/perm-change) — future wormhole-mapper changes.
- `admin_map_ownership_changed`, `admin_map_hard_deleted`, `admin_acl_ownership_changed`, `admin_acl_hard_deleted` — future admin-override paths.

Rejected alternative: ship only the v1-emitted variants and add dormant ones lazily. Loses the catalogue stability win and means every feature change has to extend the enum *and* think about whether any existing emit-call needs updating to use a new variant.

### Decision: Two renames from the older iteration

- `ghost_character_claimed` → `orphan_character_claimed`. The current codebase uses "orphan" everywhere (`db/accounts.rs` docstring, `eve-sso-auth` spec, `account-management` spec). "Ghost" was that iteration's term. Picking one and being consistent matters because `event_type` strings are an API.
- Keep `account_deletion_requested` (the older iteration's name) rather than rename to `account_soft_deleted`. Reasoning: the current product has no grace period (soft-delete is immediate, reactivation via SSO) but the older iteration's name leaves room for an `account_deleted` event when an actual hard-purge fires later. Two events ("request" + "delete") is the honest two-phase story; folding into one event would force a rename when the purge feature ships.

### Decision: Audit writes are INSERT-only forever — no UPDATE, no DELETE, no edit affordance

The application code defines no `UPDATE audit_log …` or `DELETE FROM audit_log …` query. The admin UI (in the next change) is read-only — there is no "edit" or "delete this audit entry" affordance and there will never be one.

Why this matters for the snapshot columns specifically: `actor_character_name` is denormalised data. If we ever ran a "refresh stale character names" job that touched audit rows, we'd silently rewrite history. The INSERT-only rule prevents that by construction.

We do *not* enforce this with a DB-level `TRIGGER` that raises on UPDATE/DELETE. That's overkill for the threat model (a malicious admin with DB access can do anything anyway) and adds friction for one-off DBA needs (e.g., `DROP TABLE audit_log` during a destructive schema migration would have to drop the trigger first). The invariant lives in the spec and in code review.

### Decision: Read API takes pool + filters + keyset cursor, no offset pagination

`list_audit_log(pool, event_type, actor_account_id, before, limit)` is ported almost verbatim from the older iteration:

```sql
SELECT … FROM audit_log
WHERE ($1::TEXT IS NULL        OR event_type        = $1)
  AND ($2::UUID IS NULL        OR actor_account_id  = $2)
  AND ($3::TIMESTAMPTZ IS NULL OR occurred_at       < $3)
ORDER BY occurred_at DESC
LIMIT $4
```

Three filter axes (event_type, actor_account_id, occurred_at-cursor) cover the realistic admin-browser use cases. All bound parameters — no string interpolation, no SQL injection surface.

Keyset pagination on `occurred_at DESC` (with `before` as the cursor) is correct under the INSERT-only invariant: rows never move, so cursors never get stale. Offset pagination is rejected because it skews under concurrent inserts (any new row pushes everything after the cursor down by one).

No filter on `actor_character_id` or `details->>'…'` in v1. If admin UX later needs "show all events involving EVE character X" or "show all events for account Y as target," that's an additive change: add a filter parameter, add a GIN index on `details` if performance demands it.

### Decision: Backend-only change, no frontend slice

The audit module + retrofits + tests are entirely in `backend/`. There is no admin browser UI here. Rationale:

- The admin browser UI belongs with the broader admin shell (route layout, user-menu integration, non-admin 404s) that lands in `add-server-admin-and-block-list`. Splitting the admin UI across two changes would force scaffolding twice.
- The audit data starts accumulating from the moment this change lands. By the time the admin UI ships, there is real audit history to browse — which makes the UI integration test bed richer.
- This change touches only backend files, so its verification step uses only the backend commands (`cargo fmt --check`, `cargo clippy`, `cargo sqlx prepare`, `cargo test`, hurl smoke). The frontend's three-command verification rule does not apply.

### Decision: Service signatures widen where needed to thread `tx` through

Most existing services already use a `Transaction<'_, Postgres>` internally. A small number do not — they call a single DB helper that takes `&PgPool`. Those will be widened so the audit emission and the state change land in one transaction.

Specifically (likely list, finalised during implementation):

- `services/account.rs::delete_account` — already transactional (per `clear-tokens-on-soft-delete` change), audit emit slots in cleanly.
- `services/account.rs::set_main_character` — currently calls `db::characters::set_main(&mut tx, …)` inside a transaction; audit emit slots in.
- `services/account.rs::delete_character` — review and widen if needed.
- `services/api_keys.rs::create_api_key` and `revoke_api_key` — review and widen if needed.
- `services/auth.rs` (SSO callback's `complete_login` path) — already heavily transactional; audit emits slot into the existing tx after `promote_if_no_main`.

No new public API on `db/*` modules beyond `get_main_for_account_tx`. No changes to handler signatures (the audit emission is invisible from outside the service layer).

### Decision: `ON DELETE SET NULL` on the actor_account_id FK

The older iteration's choice, kept. Rationale: when an account row goes away (today: never; in the future: hard-delete-after-grace), the audit row should preserve the *historical event* but lose the live pointer. The snapshot character columns preserve human-readable attribution.

Rejected alternatives:
- `ON DELETE NO ACTION` — blocks account deletion entirely. Becomes an obstacle when the purge feature ships.
- `ON DELETE CASCADE` — destroys audit history when accounts are purged, defeating the purpose.
- No FK at all — loses the integrity check that today's `actor_account_id` references an existing account.

## Risks / Trade-offs

- **Audit table grows unbounded.** A small community tool with ~handful of mutating actions per day produces ~thousands of rows per year. Indexes are partial, so size impact on hot paths is minimal. If this app grows to a scale where the table size matters, a future change can add retention. Mitigation: this risk is documented and explicitly accepted; the JSONB column means even at "many millions of rows" pgsql handles the access patterns fine.
- **`actor_character_name` denormalisation can confuse a reader who expects live data.** The spec scenario "snapshot reflects name at write time, not current ESI state" is explicit. Mitigation: admin browser UI (next change) will display the actor character without joining to live `eve_character` rows, so the displayed name is the snapshot.
- **Main-lookup fail-soft path can mask SSO-callback ordering bugs.** If the audit emit fires before `promote_if_no_main` in the SSO callback, character columns silently land as NULL. Mitigation: an integration test asserts that the registration audit row has non-NULL character columns for a freshly registered account — failing test surfaces the bug. Plus the `tracing::error!` is loud at runtime.
- **Dormant variants in the enum could rot.** A reader sees variants that nothing emits and may wonder if they're dead. Mitigation: a code comment on the enum (and a section in this design doc) explains the catalogue-stabilisation rationale. Each variant has unit tests covering its shape, so even unemitted variants don't silently break.
- **Lookup-inside-record_in_tx couples the audit module to `db::characters`.** Audit code transitively depends on the characters table. Mitigation: the dependency is narrow (one `SELECT eve_character_id, name FROM eve_character WHERE account_id = $1 AND is_main = TRUE`); the audit module gains a focused dependency on a single helper. Future refactors that change the main-character notion will surface as a focused failure point.

## Migration Plan

No data migration. The `audit_log` table is new; existing rows in other tables are untouched. The new migration file (`00000000000005_create_audit_log.sql`) creates the table and three indexes; it is additive and reversible via a `DROP TABLE audit_log` follow-up if ever needed.

After the change ships, all subsequent mutating actions write audit rows. Historical actions taken before the change ships are not retroactively logged — there is no source for actor-character snapshots for past events. The audit history begins at deployment time of this change; the spec is explicit about this.

`backend/.sqlx/` cache must be regenerated after the new `sqlx::query!` invocations land (one for the table-insert in `record_in_tx`, one for the list query, one for the main-lookup helper).

## Open Questions

None. The remaining decisions (IP/UA capture, naming, fail-soft vs fail-loud, lookup location, dormant variants) were resolved during exploration with explicit confirmation from the user.
