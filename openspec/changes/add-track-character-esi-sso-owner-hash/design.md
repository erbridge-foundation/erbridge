## Context

EVE's SSO access-token JWT includes an `owner` claim — a base64 hash that CCP rotates whenever a character changes hands between EVE accounts (sale, trade, restore). It is the canonical "is this still the same human's character?" signal.

Today `handlers/auth.rs::parse_esi_jwt_claims` decodes the JWT into `EsiJwtClaims { sub, name, scp }` and drops everything else, including `owner`. The callback persistence path (`services/auth.rs`) upserts the `eve_character` row keyed by the `UNIQUE eve_character_id`. Its `ON CONFLICT` ownership guard deliberately **refuses to move a character between accounts** — once `account_id` is set, a later login by a different account cannot steal it. That guard is correct for normal operation but means a genuinely transferred character would otherwise stay welded to its previous owner forever.

Relevant existing primitives we can reuse rather than reinvent:

- `db/characters.rs::clear_tokens_for_account(tx, account_id)` — NULLs the credential columns on every character of an account (built for soft-delete).
- `db/sessions.rs::delete_for_account(...)` (via `SessionStore::delete_for_account`) — removes all session rows for an account.
- `db/characters.rs::create_orphan` and the orphan-claim branch of the upsert — the existing mechanism for a character with `account_id = NULL`.
- The audit module's dormant-variant house style for events not yet emitted.

The flow runs inside the callback's single transaction, with a per-handler `AuthenticatedAccount` model (no router-tree middleware).

## Goals / Non-Goals

**Goals:**

- Persist the `owner` claim as `eve_character.owner_hash` on every successful callback.
- Detect an owner-hash change against the stored value and treat it as a character transfer.
- On transfer, sever the previous owner's access: wipe the credential columns on **all** of their characters and delete **all** of their session rows, forcing a full re-auth.
- Detach the transferred character (to the existing orphan state) and let the normal claim path re-link it to the authenticating account with fresh tokens.
- Record the transfer in the audit log.
- Keep the change backend-only.

**Non-Goals:**

- No cleanup of the previous owner's map/ACL entries that referenced the transferred character (separate follow-up). The audit event surfaces the transfer meanwhile.
- No in-app notice to the previous owner. They performed the transfer deliberately; being signed out is self-explanatory.
- No JWKS signature validation of the access token (unchanged from today — ESI tokens are trusted post-exchange).
- No proactive revocation of a lingering refresh token before the next login (see Risks).

## Decisions

### Detect on the existing `account_id`-set branch; act before the upsert

The owner-hash comparison happens only when a row already exists with `account_id` set and the stored `owner_hash` is non-null and differs from the presented claim. First-seen rows and orphan rows have nothing to compare against and simply record the claim. When a change is detected, enforcement runs **before** the upsert, and the final step sets `account_id = NULL`, which converts the situation into the already-specified orphan-claim case — so the existing claim path re-links the character with no special-casing.

*Alternative considered — reassign `account_id` old→new in place.* Rejected: it would require overriding the upsert's ownership `CASE` guard, and it risks the previous owner's references silently inheriting to the new owner. Detach-to-NULL reuses a path the system already handles.

### Sever the whole previous account, not just the transferred character

A detected transfer wipes credentials on **all** of the previous owner's characters and clears **all** of their sessions, not only the transferred one. Rationale: an owner-hash change is a strong signal that the previous owner's control of this identity is gone; treating the account as compromised and forcing a clean re-auth is the conservative, auditable choice. It also mirrors the existing soft-delete token-wipe behaviour, so it is consistent with the codebase rather than a novel pattern.

*Alternative considered — wipe only the transferred character.* Rejected for this security-driven change; it leaves the previous owner partially authenticated on an account that just lost a character to transfer.

### Clear only the previous owner's sessions

The authenticating account (the new owner) is mid-login and receives a fresh session through the normal flow; only the previous owner's sessions are deleted. There is no reason to bounce the legitimate new owner.

### New audit variant `CharacterTransferDetected`

A dedicated audit event `{ eve_character_id, old_account_id, new_account_id }` is added in the existing dormant-variant style. It is the durable record of the transfer and the seam a future ACL-cleanup change can hang off.

### Schema: nullable `owner_hash TEXT` on `eve_character`

Nullable so existing rows migrate cleanly. A null stored hash is treated as "unknown" — never a transfer — so previously-linked characters are not falsely flagged on their first post-migration login; that login records the hash for future comparisons.

## Risks / Trade-offs

- **The check only fires on re-auth.** [Risk] It catches the new owner arriving, not the previous owner's still-valid refresh token continuing to work until it next refreshes or expires. → Mitigation: wiping the previous owner's credential columns on transfer removes the stored refresh token from our side; the residual exposure is any access token already minted, bounded by its short lifetime. Full proactive revocation is out of scope.
- **Whole-account severance is aggressive.** [Risk] One character's transfer logs the previous owner out of all their characters and forces re-auth of each. → Mitigation: deliberate and documented; matches the soft-delete pattern. The previous owner re-auths normally; first-character-promotes-to-main restores their main.
- **Null/legacy owner hashes.** [Risk] Rows that predate the column have `owner_hash IS NULL`. → Mitigation: treat null as "no comparison" — record on next login, never treat as a transfer.
- **Self-retransfer / hash returning.** [Risk] If a character returns to a previous owner, that is simply another owner-hash change and re-runs the same enforcement against whoever currently holds the row. No special handling needed.
- **sqlx offline cache drift.** [Risk] New/changed `sqlx::query!` invocations break CI if the cache is stale. → Mitigation: regenerate with `cargo sqlx prepare -- --all-targets` and commit the `.sqlx/` diff.

## Migration Plan

1. Add migration: `ALTER TABLE eve_character ADD COLUMN owner_hash TEXT;` (nullable, no default — existing rows stay null until next login).
2. Deploy backend; the first login of each character backfills its `owner_hash`. No data backfill job required.
3. Rollback: dropping the column is safe — the enforcement is inert when the column/claim are absent, and no other table references it.

## Open Questions

- None blocking. The map/ACL cleanup for a transferred character is acknowledged as a deliberate follow-up, not an open question for this change.

## Future Work (out of scope here — separate changes)

This change is the **login-time** safety net: it detects a transfer only when someone re-authenticates. Two follow-ups extend coverage, in dependency order:

1. **Proactive token refresh while active.** Refresh a character's access token on logon and on a cadence *while the owner is active*; the refreshed JWT re-exposes the `owner` claim, so the same owner-hash compare runs proactively. On refresh failure (CCP revokes all app authorisations on transfer, so the stored refresh token dies at the source), mark the token expired / NULL the credential columns and require re-auth — do **not** auto-detach (a dead token alone is not proof of transfer; could be a CCP outage or plain expiry). Depends on a notion of "active".
2. **SSE presence/event channel** — the dependency under #1. "Active" = holding an open authenticated SSE connection. Scope the eventual change to a **presence channel + a `publish(event, audience)` dispatch seam** (an `EventDispatcher` trait with a `Local` in-memory implementation), explicitly **not** a cross-instance event bus. The bus is a *future implementation of that trait*, built only when real horizontal scale-out is decided — designing it now would lock in distributed-systems choices with the least information. Producers must call `publish(...)`, never touch the connection registry directly, so multi-instance later is a trait swap, not a rewrite. Note: multi-instance also breaks the current in-memory `SessionStore` (`tokio::sync::RwLock`) — a separate today-problem to resolve before scale-out.

Maps will be the first heavy consumer of the SSE channel; the channel should land on its own merits, not as a sub-feature of maps or token-refresh.
