## Context

ERB authenticates exclusively through EVE SSO. An account's identity is **derived** entirely from its characters: "which account is this" is computed as "the account owning the character whose `is_main = TRUE`." The SSO access-token JWT carries an `owner` hash that CCP rotates whenever a character is transferred to a different EVE account; ERB already captures it as `eve_character.owner_hash` (added by `add-track-character-esi-sso-owner-hash`) but uses it only in the daily token-refresh sweep, which passively flags `token_status = owner_mismatch`.

The bind path (`services::auth::complete_sso_callback`, covering both login and add-character) keys every decision on `eve_character_id` alone. So a transferred character produces a wrong outcome: on login the new owner is dropped into the seller's account (`resolve_or_create` returns `ExistingAccount`); on add-character the add is hard-rejected `BoundElsewhere`. Both ignore the one signal that distinguishes "still the seller's character" from "legitimately transferred to this human": the owner hash.

Relevant existing code:
- `services/auth.rs::complete_sso_callback` — owns the bind transaction; already receives `owner_hash` in `SsoCompletionInput`.
- `db/accounts.rs::resolve_or_create` — login-mode account resolution (keyed on `eve_character_id`).
- `db/characters.rs::find_account_id_for_eve_character` — `Option<Option<Uuid>>` lookup used by the add-character `BoundElsewhere` branch.
- `db/characters.rs::set_main` (returns the row) and `promote_if_no_main` (returns only a bool) — the two `is_main = TRUE` writers.
- `db/accounts.rs::soft_delete` — user-facing delete; FK graph in migrations 1/2/5/7/9 (CASCADE for account-private rows, SET NULL for co-owned rows + audit actor).

## Goals / Non-Goals

**Goals:**
- Detect a transferred character at bind time (login AND add-character) via owner-hash comparison and re-home it to the authenticating owner instead of mis-binding or rejecting.
- Keep an emptied (zero-character) seller account identifiable and non-deletable-by-accident via a denormalized `last_known_main_*` snapshot and a new terminal `'orphaned'` status.
- Provide a narrow admin hard-delete (with a deletion preview) as the in-app resolution for the one dead-end this feature's conditions can create.
- Preserve all existing safe-by-default behaviour: never detach on absent/unprovable evidence; matching-hash same-account login stays a normal self-heal.

**Non-Goals:**
- Account merge / admin character-move between a human's own accounts (the inverse, matching-hash, same-human re-homing problem). Explicitly deferred.
- Self-service character release from an account.
- Any change to the daily sweep's passive `owner_mismatch` flagging; the two mechanisms coexist.
- EVE-side token revocation.

## Decisions

### Decision: A single transfer predicate, both call sites, inside the bind transaction
A character is **transferred** iff: presented `owner` hash present **AND** stored `owner_hash` non-null **AND** they differ. Anything else (absent presented hash, null stored hash, matching hashes) is *not* a transfer and falls through to existing behaviour. In Rust this is `existing.owner_hash.as_deref() == Some(presented)` for "same human"; `None`/null stored naturally yields "not a transfer," so no special NULL branch is needed.

Why both call sites in one place: login and add-character differ only in *where the character lands* (a freshly-resolved account vs the session account). The detect → detach → seller-fixup → audit logic is identical. Placing the check ahead of the resolve/bind decision in `complete_sso_callback` avoids duplicating it. Evaluating it inside the existing SSO-completion transaction reuses the race-safety the `BoundElsewhere` check already relies on.

_Alternative considered_: only fix add-character now, defer login. Rejected — login is the user's *original* reported bug (buyer lands in seller's account), and the shared logic makes "both" barely more than "one."

_Alternative considered_: act on the bind only when the row is already flagged `owner_mismatch` by the sweep. Rejected — that makes correctness depend on the sweep having run first; the presented hash is authoritative on its own.

### Decision: Rebind in place (not orphan-then-claim)
On transfer, `UPDATE eve_character SET account_id = <destination>, tokens/hash/scopes/public-info, token_status='valid'` directly — the destination is already known (session account, or the resolved/new login account). No round-trip through an orphan (`account_id = NULL`) state.

_Alternative considered_: null the binding and reuse the existing orphan-claim path. Rejected — introduces a transient orphan state for no benefit when the destination is known.

### Decision: Seller-side fixup — re-promote or orphan, never delete
After detaching, in the same transaction:
- remaining characters > 0 and the detached char was the main → promote a remaining character (and update the seller account's `last_known_main_*`);
- remaining characters > 0 and the detached char was not the main → nothing (main intact);
- remaining characters == 0 → set seller `status = 'orphaned'` (keep the row).

We do **not** hard-delete the seller here. Auto-deleting an account as a side effect of someone else's login is surprising and irreversible; orphaning keeps the row, its `last_known_main_*` label, and its owned maps/ACLs visible to admins. (Hard-delete exists as a deliberate admin action, below — not an automatic side effect.)

### Decision: Denormalize the main onto the account (`last_known_main_*`), maintained at every `is_main` flip
A zero-character account is unnameable because identity is derived from `is_main`. Storing `last_known_main_character_id` (BIGINT, **not** an FK) + `last_known_main_character_name` (TEXT) on `account` keeps it nameable after emptying. This mirrors the project's [[project-audit-self-contained-names]] decision: snapshot at write time, never resolve at read.

It is **not** a join key — `is_main` remains the single source of truth for the live main. The snapshot is written in-tx at the two and only two `is_main = TRUE` writers: `set_main` (already returns the row → trivial) and `promote_if_no_main` (returns only a bool → the caller passes `eve_character_id` + name, which `SsoCompletionInput` already holds).

_Alternative considered_: make it a FK with `ON DELETE SET NULL`. Rejected — the whole point is to survive the character row leaving; a nullable FK would null exactly when we need the value.

### Decision: New terminal `'orphaned'` status, distinct from `'soft_deleted'`
`soft_deleted` carries the contract "owner can log back in to reactivate." A zero-character account *cannot* be reactivated (no character can resolve to it), so reusing `soft_deleted` would be a standing lie and could let reactivation logic mistakenly treat it as recoverable. A distinct `'orphaned'` keeps the state machine honest. Cost: extend the `account.status` CHECK/domain and any status enumerations.

### Decision: Narrow admin hard-delete with a deletion preview
A real `DELETE FROM account` behind `AdminAccount`, reusing the soft-delete last-admin guard, emitting an audit event. The FK graph already defines the entire blast radius — `eve_character`/`session`/`api_key` CASCADE; `map`/`acl` owner, `audit_log.actor`, `blocked_eve_character.blocked_by` SET NULL — so the delete is bounded and safe. The one new surface is a **preview** (counts of removed rows + maps/ACLs that go unowned). Audit history is **preserved** (snapshot actor character id/name + self-contained JSONB details survive the actor null), so the preview must not describe audit as lost.

Scoped into this change rather than spun out because, without it, the documented dupe-account dead-end has no in-app exit (today's user delete is soft-only and retains characters). Kept deliberately narrow — no merge, no character-move, no bulk tools.

### Decision: Document the dupe-account collision as a known limitation
A buyer who already has an ERB account but logs in *fresh* with the transferred character gets a *second* account, because ERB has no identity link between two accounts beyond the character itself. This is pre-existing ERB behaviour (any never-seen character mints a new account), not something this change worsens. Resolution: admin hard-deletes the spare; the human then adds the character to their real account. Documented, not auto-solved.

## Risks / Trade-offs

- **[False-positive detach on a hash we misread]** → The predicate requires a *present* presented hash AND a *non-null* stored hash that *differ*; any ambiguity falls back to the conservative `BoundElsewhere`/normal path. Owner hash is stable across re-logins for the same character on the same EVE account, so a differing pair is a strong, low-false-positive signal. The cost of a wrong *reject* (annoyance, retry, admin fix) is far lower than a wrong *detach* (data movement), so the predicate is deliberately biased toward not-detaching.
- **[Auto-orphaning the seller surprises them]** → Orphaning is non-destructive (row + label + owned resources retained); the seller's recourse (re-auth a remaining character, or admin attention) is intact. We chose orphan over delete precisely to avoid irreversible side effects of a third party's login.
- **[`'orphaned'` status missed by an existing status check]** → Audit every place that branches on `account.status` (auth gating, admin listing, soft-delete reactivation) when adding the value; the spec marks it BREAKING for status enumerators.
- **[Hard-delete is irreversible]** → Gated by `AdminAccount` + last-admin guard + a blast-radius preview + explicit confirm; FK graph bounds the fallout; audit records the deletion with the `last_known_main_character_name` snapshot.
- **[Owner-less maps/ACLs after delete or orphan]** → Out of scope to auto-reassign; they remain visible to admins (SET NULL, not cascade). A future change may add reassignment. Flagged so it is a conscious non-goal, not a surprise.
- **[sqlx offline cache drift]** → Regenerate `.sqlx/` after the schema/query changes and commit the diff (project tooling requirement).

## Migration Plan

1. New migration: add `account.last_known_main_character_id BIGINT NULL`, `account.last_known_main_character_name TEXT NULL`; extend the `account.status` CHECK/domain to include `'orphaned'`.
2. Backfill `last_known_main_*` for existing accounts from their current `is_main = TRUE` character (one-time UPDATE in the migration) so existing accounts are immediately nameable.
3. No data backfill needed for `'orphaned'` — it is only ever reached by the new detach path going forward.
4. Regenerate the sqlx offline cache; commit `.sqlx/`.
5. Rollback: the feature is additive (new columns, new status value, new bind branch, new admin endpoint). Reverting the code restores prior bind behaviour; the new columns/status value can remain unused without harm.

## Open Questions

- None blocking. (Owner-less-resource reassignment and account-merge are explicit out-of-scope follow-ups, not open questions for this change.)
