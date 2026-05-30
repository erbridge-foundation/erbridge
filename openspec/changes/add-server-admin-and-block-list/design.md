## Context

Admin power today is a single bootstrap rule: the first account to complete SSO gets `is_server_admin = TRUE` (in `accounts::resolve_or_create`). There is no grant, no revoke, and no way to ban a pilot. The `add-audit-log` change (already shipped) deliberately seeded dormant `AuditEvent` variants for exactly this work: `server_admin_granted{admin_grant}`, `server_admin_revoked`, `eve_character_blocked`, `eve_character_unblocked`. This change activates them and adds the surrounding capability.

A prior iteration of this codebase (`zz-ref/backend/older-iteration/`) shipped a complete admin + block-list feature. It is the reference for *logic* (last-admin guard, idempotent grant, block-bans-account) but **not** for *shape*: it enforced via router-tree middleware layers (`require_server_admin`, `require_active_account`), whereas this codebase deliberately uses per-handler extractors (see memory `project-backend-auth-model` and the existing `AuthenticatedAccount`). We keep the extractor model.

The design below was settled in an explore session; the key insight that shaped enforcement is recorded under "Decision: Block enforcement mirrors soft-delete, not a per-request check."

## Goals / Non-Goals

**Goals**

- Grant and revoke `is_server_admin` on any account, with a last-admin guard that prevents removing the final active admin and a self-block guard that prevents an admin locking themselves out via the block list.
- A character block list keyed on the immutable EVE character ID, self-contained (denormalised name/corp snapshot) so unknown pilots can be pre-emptively blocked and the list never joins to `eve_character`.
- Blocking a character bans the whole owning account, with the same credential-destruction semantics as soft-delete (tokens cleared, sessions deleted) applied to the owning account.
- Block enforcement that costs nothing on the hot session-cookie path, by reusing the soft-delete enforcement model (block destroys the session; SSO refuses a new one; the surviving bearer route is checked).
- Admin-only HTTP surface gated by a cookie-only `AdminAccount` extractor with a fail-closed coverage test, plus a frontend `/admin` shell that 404s non-admins.
- An admin audit browser over `audit::list_audit_log`, consuming its target-first filter axis (`target_type`/`target_id`/`target_name`, added by `add-audit-log-target-columns`) so "who did X to whom" is an indexed query rather than a JSONB scan.

**Non-Goals**

- Map/ACL admin overrides (`admin_map_*`, `admin_acl_*` audit variants stay dormant until maps/ACLs exist).
- Account hard-purge/restore (`account_purged` stays dormant).
- Admin authentication via API key — admin actions are session-cookie only in v1.
- Time-boxed or auto-expiring blocks — blocks are indefinite until unblocked.
- Corp/alliance-level blocks — v1 blocks individual characters only (the schema does not preclude a future `target_type`, but v1 does not build it).

## Decisions

### Decision: Block enforcement mirrors soft-delete, not a per-request check

This is the load-bearing decision. The naive worry was "every authenticated request must check the block list, and `/me` is on the hot path at thousands-of-characters scale." Inspecting the existing extractor dissolved that worry.

The current `AuthenticatedAccount` extractor handles soft-delete asymmetrically and correctly:

- **Cookie path**: resolves the session → returns `account_id`. It does **not** check `soft_deleted`, because soft-delete calls `session_store.remove_all_for_account`, so a soft-deleted account has no live session to present. The session deletion *is* the enforcement.
- **Bearer path**: API keys survive soft-delete (per the account-management spec), so the bearer branch is the one surviving auth route — and it is the only branch that fetches the account row and checks `status == 'soft_deleted'`.

Block adopts this model exactly:

```
Block action (admin), in one transaction:
  • insert blocked_eve_character row
  • IF the blocked character resolves to an account A:
      - clear A's EVE tokens (same columns as soft-delete)
      - delete all of A's sessions  → cookie path is now dead for A
  • emit eve_character_blocked

SSO callback:
  • check the block list for the resolved eve_character_id
  • if blocked → reject (no account write, no session); emit blocked_login_rejected
  • runs for BOTH the login flow and the add-character flow

Bearer branch of AuthenticatedAccount:
  • already fetches the account row + checks soft_deleted
  • ADD: account_has_blocked_character join → reject with account_blocked

Cookie path:
  • NO block check — a blocked account has no live session (identical to soft-delete)
```

Consequence: the hot `/me`-via-cookie path gets zero new work, even at scale. Enforcement lives at session-creation (SSO) and on the surviving bearer route only. No new `ActiveAccount` extractor, no middleware layer.

Rejected alternatives:
- **Check block on every request** (fold into both extractor branches, or a denormalised `account.is_blocked` flag read on every request): over-engineered. The soft-delete model proves the cookie path needs no status/block check because the session is already gone. Adding one would be inconsistent with how soft-delete already works and would tax the hottest path for no benefit.
- **Middleware layer** (`require_active_account` like the older iteration): conflicts with the per-handler-extractor auth model this codebase committed to.

### Decision: Bearer-path block check is a join, not a denormalised flag

The bearer branch already fetches the account row, so a denormalised `account.is_blocked` boolean would give a free check. We use a **join** (`account_has_blocked_character(account_id)`) instead, because:

- Bearer auth is low-volume (API automation), so the extra indexed query is irrelevant.
- The flag would be denormalised state with a non-trivial invariant — "`is_blocked` iff the account owns any blocked character" — that must be maintained on block, unblock, and any change to which account owns a character. The join computes the truth directly with no invariant to drift.

Rejected: the flag. Premature optimisation for a non-hot path; trades a clean derived query for maintenance burden.

### Decision: blocked_eve_character is a self-contained snapshot — no FK to eve_character

The block row stores `character_name` and `corporation_name` denormalised, with **no foreign key** to `eve_character`. Reasoning:

- **Pre-emptive blocking.** Admins must be able to block a known griefer who has never signed in (no `eve_character` row exists). A FK would make that impossible (insert would violate the constraint). This was the older iteration's limitation — its block table had the FK and could only block already-seen characters.
- **Self-contained list.** The admin block list reads flat — no join to `eve_character` to get a display name. The row carries everything the UI needs.
- **Snapshot is correct, not just convenient.** CCP does not allow player-initiated character renames (only CCP support changes a name, exceptionally, e.g. an offensive name); the EVE character ID is immutable. So the denormalised name is effectively permanent-correct. Even in the rare CCP-rename case, "the name at time of block" is the right thing to show (it is what the admin saw when deciding).

To populate the snapshot, the block endpoint fetches ESI public-info (name + corporation) best-effort. **A block SHALL succeed even if ESI is unavailable** — the row is inserted with null name/corp and the block is fully effective (enforcement keys on the ID, not the name). A block is a security action and must not be gated on CCP's API being up.

Rejected alternatives:
- **FK + only-already-seen** (older iteration): cannot pre-emptively block; rejected.
- **ESI-fetch + create an orphan `eve_character` row, then block** (FK retained): pollutes `eve_character` with rows that exist only to be blocked, and forces a decision about orphan cleanup on unblock. The self-contained snapshot sidesteps both.

### Decision: Blocking clears tokens and tears down the owning account, like soft-delete

When a blocked character resolves to an account, the block transaction clears that account's EVE tokens (the same four columns soft-delete clears: `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `scopes`) and deletes all its sessions. Rationale: the security principle established by `clear-tokens-on-soft-delete` — "the service holds no usable EVE credentials for an account that cannot use the service" — applies equally to a blocked account. Symmetry with soft-delete also gives a single mental model.

Cost accepted: on unblock, every character of the formerly-blocked account reports `token_status = "expired"` and must be re-SSO'd individually. This is the same friction users already tolerate for ordinary token expiry, and it is the admin's (reversible) decision. The whole *account* is torn down, not just the one blocked character, because the account — not the character — is what is banned (one blocked character ⇒ account banned).

If the blocked character does not resolve to an account (orphan row, or never seen), there is nothing to tear down; the block row is recorded and the SSO callback will reject any future login.

### Decision: AdminAccount extractor — cookie-only, per-handler, fail-closed coverage test

A new `AdminAccount(Uuid)` extractor mirrors `AuthenticatedAccount`:

- **Session-cookie only.** It does not accept `Authorization: Bearer erb_…`. A leaked API key must never be a persistent server-takeover vector; an admin action requires a fresh-ish (7-day sliding) browser session. The first-class story for server automation remains the existing (unused) `scope = 'server'` API key, not admin user-actions.
- **Per-handler.** Each admin handler declares `AdminAccount(_)` in its signature. A handler that forgets it is simply not admin-gated — identical fail-open-by-omission risk to `AuthenticatedAccount`, mitigated the same way: an `admin_auth_coverage` test asserts every registered `/api/v1/admin/*` route extracts `AdminAccount`, mirroring the existing v1 auth-coverage test.
- Rejects unauthenticated with 401, authenticated-but-not-admin with 403 `forbidden_admin_required`.

Rejected: a router-tree middleware layer (older iteration's `require_server_admin`) — conflicts with the per-handler model.

### Decision: blocked_login_rejected is a new audit variant (recording an attempt)

The audit log's stated philosophy is "records committed state changes." A rejected login is a *non-event* in that sense — nothing was written. We add `BlockedLoginRejected { eve_character_id }` (actor null; the rejected character lives in `details`, not the actor column, because they were the *subject* of the rejection, not an actor) anyway, deliberately bending the model, because:

- It is the only durable store we have, and "is this blocked pilot still trying to get in?" is a genuine admin question for a community tool.
- It is low-volume and security-relevant.

This is the first variant that records an attempt rather than a change. We accept that, narrowly, for this one security-relevant case. (The user chose this over the purist alternative of `tracing::warn!`-only.)

### Decision: Grant by account UUID; resolve via a dedicated character-search endpoint

The grant/revoke endpoints take an **account UUID** in the path. Accounts have no user-facing name, so the frontend resolves "promote the account that owns *Pilot X*" via `GET /api/v1/admin/characters/search?q=`, which matches a name fragment against `eve_character` and returns characters with their owning `account_id`. The admin picks a character; the frontend POSTs grant to that account's UUID.

A dedicated search endpoint (rather than the older iteration's "list all accounts and filter client-side") is justified by the explicit scaling target: hundreds of users, thousands of characters. Listing every account to populate a picker does not scale; an indexed name search does.

### Decision: Block relies on session deletion as a universal revocation signal — binding on future long-lived connections

Block enforcement (above) destroys the owning account's sessions. That session row is the natural thing any session-validating consumer re-checks — REST today, SSE / websocket tomorrow. Because block (like soft-delete and logout) *deletes* the session row rather than setting a checked-per-request flag, the revocation is a universal signal: any consumer that re-reads the session discovers it, without the block code needing to know that consumer exists.

This only delivers "near-instant" block for streaming endpoints **if those endpoints periodically re-validate the session** instead of authenticating once and streaming forever. A stream-forever connection that ran `AuthenticatedAccount` once at connect would keep emitting after a block, because nothing re-reads the (now-absent) session. We therefore make a forward-compatibility rule **normative in the `server-administration` spec**: a long-lived authenticated connection SHALL periodically re-validate its session against the session store, and SHALL NOT authenticate-once-and-stream-forever. A compliant connection closes within one heartbeat of a block; the client's automatic reconnect re-runs `AuthenticatedAccount` (cookie: session gone → 401; the block also prevents a fresh SSO session), and the distinct `account_blocked` error routes a reconnecting blocked client to `/blocked` rather than the login page. Block latency for a compliant streaming endpoint is therefore one heartbeat interval — effectively instant.

**Heartbeat synergy.** This costs the future SSE change nothing extra, because the heartbeat it needs for block-discovery is the *same* mechanism that keeps the session alive. Sessions have a sliding 7-day expiry refreshed (`last_seen_at = now()`, `expires_at = now() + 7d`) on every authenticated request. An SSE heartbeat that touches the session store to refresh the sliding window is, in the same round-trip, the moment it discovers a deletion. The future change should reuse that one mechanism — refresh-and-revalidate on a heartbeat — rather than inventing a separate block-polling path.

Why normative rather than a soft note: SSE will be built months after this change, by which point the enforcement model's reliance on session deletion is easy to forget. A `SHALL` line a reviewer can cite is what turns "block is instant" from an accident into a guaranteed property. It also explains *why* we did not add a per-request block check to the hot cookie path — the streaming requirement, not a cookie-path check, is what makes long-lived connections safe.

### Decision: Idempotent grant/block, last-admin and self-block guards in-transaction

- **Grant** is idempotent: granting an already-admin account is a success no-op (no audit event). **Block** is idempotent: blocking an already-blocked character is a success no-op (no audit event). Mirrors the older iteration.
- **Revoke** runs the last-admin guard inside the transaction: if revoking would drop the active-admin count to zero, it returns 409 `cannot_remove_last_server_admin` and rolls back. Self-revoke is permitted otherwise. (`count_server_admins` already exists from the bootstrap work.)
- **Block** runs a self-block guard: an admin SHALL NOT block any character belonging to their own account (would let an admin lock themselves — and possibly the server, if they are the last admin — out). Returns a 4xx (`cannot_block_self`) and writes nothing.

## Risks / Trade-offs

- **[Block evasion via add-character]** A blocked pilot could try to become an existing account's alt to slip past a login-only check. → Mitigated: the SSO block check runs for the add-character flow too, so a blocked character can never be attached to any account.
- **[Unblock re-auth friction]** Unblocking leaves every character of the account `token_status = "expired"`. → Accepted: same friction as ordinary token expiry; the block was the admin's reversible decision; matches the soft-delete trade.
- **[ESI down at block time]** The name/corp snapshot can't be fetched. → Mitigated by design: the block still succeeds with null name/corp; enforcement keys on the immutable ID.
- **[Audit model bent by blocked_login_rejected]** Recording an attempt departs from "state changes only." → Accepted narrowly for one security-relevant, low-volume case; documented as such.
- **[Fail-open by extractor omission]** A new admin handler that forgets `AdminAccount` is ungated. → Mitigated by the `admin_auth_coverage` test, exactly as the existing auth-coverage test mitigates the same risk for `AuthenticatedAccount`.
- **[Bearer-path latency]** The block-list join adds a query to bearer auth. → Negligible: bearer auth is low-volume and the join is indexed; the hot cookie path is untouched.
- **[Self-lockout via revoke + block interplay]** Last-admin guard covers revoke; self-block guard covers block. The two guards together prevent an admin removing their own access by either axis. A determined last admin can still revoke a *second* admin and then has no one to restore them — but that is inherent to single-tenant admin and out of scope to solve here.
- **[Stream-forever connection outlives a block]** A future long-lived connection that authenticates once and never re-reads its session would keep streaming after the account is blocked. → Mitigated by a normative requirement (see the session-deletion decision): streaming endpoints SHALL re-validate the session on a heartbeat and SHALL NOT authenticate-once-and-stream-forever, bounding block latency to one heartbeat. This change introduces no streaming endpoint; the requirement binds the future one.

## Migration Plan

No data migration. New migration `00000000000006_create_blocked_eve_character.sql` creates the `blocked_eve_character` table; additive and reversible via `DROP TABLE`. The `.sqlx/` cache is regenerated for the new queries. Existing accounts are unaffected until an admin acts. The bootstrap admin remains the bootstrap admin; this change only adds the ability to grant/revoke beyond it.

## Open Questions

None. The four explore-session questions (token-clear on block, blocking unknowns, FK, blocked_login_rejected) plus the enforcement-location and self-block questions were all resolved with the user. The frontend design (page layout, search UX detail) is conventional and deferred to implementation under the `sveltekit-node` skill.
