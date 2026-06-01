## Context

EVE's SSO access-token JWT includes an `owner` claim — a base64 hash CCP rotates whenever a character changes hands between EVE accounts (sale, trade, restore). CCP's ESI best-practices documentation names it the canonical ownership signal and instructs applications to use it to "disassociate [a character] with the prior owner's login" when it changes.

**Key fact established during exploration (from CCP docs):** CCP documents *no* automatic revocation of refresh tokens on transfer. A refresh token "has no expiration unless revoked," and the only documented cause of invalidation is the application revoking it manually. Therefore a sold character's refresh token may keep producing valid access tokens under the new owner — refresh *failure* is **not** a reliable transfer signal. The owner-hash change is the only reliable one. (This is why mature tools such as Wanderer store the hash.)

Today `handlers/auth.rs::parse_esi_jwt_claims` decodes the JWT into `EsiJwtClaims { sub, name, scp }` and drops `owner`. The token-refresh path `esi/token.rs::refresh_access_token` returns `RefreshedTokens { access_token, refresh_token, access_token_expires_at }` and likewise drops `owner`, even though the refreshed access token is itself a JWT carrying it.

The codebase has **no background-task infrastructure** — `main.rs` builds the router and calls `axum::serve`; there is no `tokio::spawn`, interval, or scheduler anywhere. This change introduces the first one.

Reusable primitives:
- `esi/token.rs::refresh_access_token` — OAuth2 refresh-token grant; failure is non-fatal by design.
- `db/characters.rs::update_tokens_by_eve_id` — writes refreshed tokens (currently bumps the noisy `updated_at`).
- `db/characters.rs::clear_tokens_for_account` — NULLs credential columns (the existing "must re-auth" convention).
- The audit module's dormant-variant house style (`MapCreated`, `AclCreated`, … added ahead of their consumers).

## Goals / Non-Goals

**Goals:**
- Capture the `owner` claim as `eve_character.owner_hash` on every successful callback and every successful background refresh.
- Detect a transfer via a **daily background sweep** that refreshes tokens and compares the hash — not at login time.
- Represent token health as a three-state `token_status` (`valid` / `token_expired` / `owner_mismatch`) that drives UI guidance.
- Force re-authentication of stale credentials via a 7-day account-idle waterfall.
- Let a matching-hash auth self-heal any non-`valid` state.

**Non-Goals:**
- **No login-time enforcement / account severance.** The original synchronous "sever the previous owner in the callback transaction" design is replaced; detection is moved out of the login path entirely.
- No automatic detach/delete of an `owner_mismatch` character. It is flagged; removal is user- or admin-initiated.
- No proactive/while-active refresh and **no SSE dependency** — the daily cadence needs no notion of "active".
- No JWKS signature validation of access tokens (unchanged — trusted post-exchange).
- No per-character idle clock. Idle is measured account-wide via `account.last_login` (deliberately blunt).

## Decisions

### Detect via a daily sweep, not at login
A character transfer takes ~24h to complete but flips the owner hash almost immediately on initiation. A once-daily sweep therefore catches the change within the transfer window, before the buyer ever logs in. Moving detection off the login path eliminates the gap the original login-time design had: in the login flow `db/accounts.rs::resolve_or_create` resolves a welded character straight back into the *previous* owner's account (there is no distinct "account B" to re-link to), so login-time detach-and-reclaim could not produce the right owner without restructuring resolution. The sweep sidesteps this entirely.

*Alternative considered — login-time enforcement (original proposal).* Rejected: it both had the resolve-into-previous-owner gap and required heavy synchronous severance in the auth hot path.

### Refresh-success + hash compare is the transfer signal; refresh-failure is only "expired"
Because CCP does not revoke refresh tokens on transfer, the sweep must actually refresh and read the *new* JWT's `owner` claim. Only a **successful refresh with a differing hash** sets `owner_mismatch` (it is proof). A **failed refresh** sets `token_expired` — we cannot read a hash from a failure, so we must not claim "sold". This keeps the `owner_mismatch` state honest: it is never a guess.

### Three states, advisory and self-healing — "a successful auth always wins"
`token_status` is a UI hint, not a lock. Any successful login or refresh presenting an owner hash that **matches** the current row resets `token_status = valid` and restores tokens. This self-heals false-positive `owner_mismatch` (CCP hiccup) and genuine re-acquisition (chars get sold back / transfers reversed) for free, and means no state is terminal. The source of truth is always "the hash from the latest successful auth," exactly as CCP intends.

`token_expired` and `owner_mismatch` differ only in the *action they imply*: `token_expired` → the legitimate owner re-logs in and it clears; `owner_mismatch` → the old owner *cannot* re-auth (hash now belongs to the buyer), so the row is removed by the user or an admin. That difference in call-to-action is the entire justification for a third state over a single boolean.

### Account-level idle clock (`account.last_login`)
The 7-day waterfall measures idle at the **account** level, not per character. A per-character "last authed" clock was considered and rejected as over-engineering: if nobody has logged into the account for 7 days, forcing a fresh login on next visit costs the legitimate user nothing, and expiring all their characters' tokens together is acceptable. `eve_character.updated_at` is unusable for this (it bumps on name/corp changes too); `session.last_seen_at` is per-session. A dedicated `account.last_login` is the clean signal.

### Schema: nullable `owner_hash`, `token_status` with CHECK, `account.last_login`
`owner_hash` nullable so legacy rows migrate cleanly; null = "not yet observed", never a transfer (the first post-migration refresh/login records it). `token_status TEXT NOT NULL DEFAULT 'valid'` with a CHECK over the three values (string + CHECK matches the existing `account.status` convention rather than a PG enum type). `account.last_login` nullable (null treated as "unknown" — the waterfall can backfill on first observation rather than mass-expiring legacy accounts).

### New dormant audit variants
`CharacterOwnerMismatch { eve_character_id, account_id }` (and optionally `CharacterTokenExpired { … }`) added in the established dormant-variant style, as the durable record and the seam a future ACL-cleanup change hangs off.

### FK-cascade invariant for future character-scoped tables
Nothing currently FKs to `eve_character.id` (the cascade graph is rooted at `account`). When map-ownership / ACL-membership tables arrive, they MUST FK to `eve_character.id ON DELETE CASCADE` (and to `account.id ON DELETE CASCADE` as today), so that removing an `owner_mismatch` character cleans up its references automatically. The audit log MUST NOT cascade (it records `eve_character_id` as a plain `i64`, already decoupled). Recorded here as a forward constraint, not implemented now.

## Risks / Trade-offs

- **Sweep load / ESI rate limits.** [Risk] Refreshing every character daily is N token-endpoint calls. → Mitigation: spread/throttle within the run; the refresh endpoint is not the rate-limited ESI data plane; daily cadence is low. Tune batch size if needed.
- **Detection latency up to ~24h.** [Risk] A transfer is caught on the next sweep, not instantly. → Accepted: the transfer itself takes ~24h, and there is no synchronous threat to gate (the buyer can't usefully act on a not-yet-linked character). Proactive/while-active refresh is the deferred follow-up if tighter latency is ever needed.
- **Null/legacy owner hashes.** [Risk] Rows predating the column have `owner_hash IS NULL`. → Treat null as "no comparison": record on next successful auth, never a transfer.
- **Single-instance background task.** [Risk] If the backend ever scales horizontally, multiple instances would each run the sweep. → Out of scope; same single-instance assumption as the current in-memory `SessionStore`. Note it when scale-out is designed.
- **sqlx offline cache drift.** [Risk] New/changed `sqlx::query!` invocations break CI if the cache is stale. → Regenerate with `cargo sqlx prepare -- --all-targets` and commit `.sqlx/`.

## Migration Plan

1. Add migration `00000000000008_*`: `owner_hash TEXT` + `token_status TEXT NOT NULL DEFAULT 'valid' CHECK (...)` on `eve_character`; `last_login TIMESTAMPTZ` on `account`.
2. Deploy backend; the sweep and normal logins backfill `owner_hash` and `last_login` over time. No data backfill job required. Existing rows are `token_status = 'valid'` by default.
3. Rollback: dropping the columns is safe — the sweep is inert without them and nothing else references them.

## Open Questions

- Whether to also emit a `CharacterTokenExpired` audit event or only `CharacterOwnerMismatch` (transfer is the security-relevant one; plain expiry is noise). Leaning mismatch-only; settle at implementation.
- Exact sweep throttling/batching parameters — tune against real token counts.

## Future Work (out of scope here — separate changes)

1. **Automatic removal + map/ACL cleanup** for an `owner_mismatch` character, once those tables exist, leaning on the FK-cascade invariant above.
2. **Proactive / while-active refresh** for sub-24h detection latency, built on an **SSE presence channel + a `publish(event, audience)` dispatch seam** (an `EventDispatcher` trait with a `Local` in-memory impl) — explicitly **not** a cross-instance bus. The daily sweep here does NOT depend on any of this; it is the standalone baseline. Multi-instance also breaks the in-memory `SessionStore` and the single-instance sweep assumption — a separate scale-out concern.
