## Why

The EVE SSO access-token JWT carries an `owner` claim — a hash CCP rotates whenever a character is transferred to a different EVE account (sold, traded, biomassed-and-restored). It is CCP's canonical "is this still the same human's character?" signal; the ESI best-practices docs direct applications to compare it and "disassociate it with the prior owner's login" when it changes. CCP does **not** document any automatic revocation of refresh tokens on transfer — a refresh token can keep minting valid access tokens for a character that has changed hands — so the owner-hash comparison is the only reliable transfer signal we have.

Today the callback parses the JWT into `EsiJwtClaims { sub, name, scp }` and discards `owner`; the periodic refresh path (`esi/token.rs`) likewise discards it. We never compare it, so a transferred-away character keeps its tokens and its linkage to the previous owner: a stale-access vector.

## What Changes

This change is a **detection-and-flagging** mechanism, not login-time enforcement. Detection happens in a background sweep, never in the user's login request.

- **Capture the owner hash.** Persist the EVE SSO `owner` claim as `eve_character.owner_hash` on every successful callback (first link, orphan-claim, re-auth) and on every successful background refresh.
- **A daily token-refresh sweep** (new background task) refreshes the stored token of every character not already `token_expired`. Per character, on each run:
  - **Refresh succeeds, owner hash matches stored** → store the rotated tokens, keep `token_status = valid`, bump `account.last_login`'s sibling freshness as applicable.
  - **Refresh succeeds, owner hash differs** → the character was transferred. Set `token_status = owner_mismatch`, NULL the credential columns. (This is the only path that yields `owner_mismatch` — a differing hash is proof; a failed refresh is not.)
  - **Refresh fails** → set `token_status = token_expired`, NULL the credential columns.
- **7-day idle waterfall.** When an account's `last_login` is older than 7 days, the sweep expires that account's still-valid character tokens (`token_status = token_expired`, NULL credentials) regardless of refresh-token longevity — a blunt freshness floor.
- **Three token states, advisory not terminal.** `valid` / `token_expired` / `owner_mismatch`. A successful auth always wins: any login (or successful refresh) presenting an owner hash that **matches** the current row resets the state to `valid` and restores tokens — self-healing a false-positive `owner_mismatch` or a re-acquired character. Nothing is permanently stuck.
- **Recovery is by re-login or removal, not forced severance.** A `token_expired` character is fixed by the legitimate owner re-authenticating. An `owner_mismatch` character cannot be re-authenticated by the old owner (the hash now belongs to the buyer); the old owner removes it themselves, or an admin removes it.
- **Audit.** The sweep emits an audit event when it flips a character to `owner_mismatch` (and, optionally, to `token_expired`), recording `eve_character_id` and the owning `account_id`.

## Capabilities

### New Capabilities
- `character-token-lifecycle`: the daily refresh sweep, the three `token_status` states and their transitions (including self-heal), the 7-day idle waterfall, and owner-hash capture/comparison as the transfer signal.

### Modified Capabilities
- `eve-sso-auth`: the callback persists `owner_hash` and resets `token_status` to `valid` on a matching-hash login (and surfaces a mismatch on the proven path).

## Impact

- **Schema** (new migration `00000000000008_*`):
  - `eve_character.owner_hash TEXT` (nullable; null = "not yet observed", never a transfer).
  - `eve_character.token_status TEXT NOT NULL DEFAULT 'valid'` with a CHECK constraint over `('valid','token_expired','owner_mismatch')`.
  - `account.last_login TIMESTAMPTZ` (account-level; bumped on login), for the 7-day waterfall.
- **Backend (Rust)**:
  - **New background task** — there is no existing scheduler/`tokio::spawn` in the codebase, so this introduces the first one: a 24h-interval task spawned at startup in `main.rs`, with its logic in a new module (per the `rust-rest-api` skill's layout). Iterates characters, calls the refresh path, applies the state transitions and the idle waterfall.
  - `esi/token.rs` — extend `RefreshedTokens` to capture the `owner` claim from the refreshed access-token JWT (today it is dropped); reuse `parse_esi_jwt_claims`.
  - `handlers/auth.rs` — add `owner` to `EsiJwtClaims`; thread it into the callback service input.
  - `services/auth.rs` — persist `owner_hash`; reset `token_status = valid` and `account.last_login = now()` on a matching-hash login.
  - `db/characters.rs` — `owner_hash` + `token_status` in the `Character` struct and all SELECTs/upserts; a function to set `token_status` (+ NULL credentials) by character; a query selecting refreshable characters (not `token_expired`, with a refresh token).
  - `db/accounts.rs` — `last_login` read/write; a query for accounts idle > 7 days.
  - `audit/mod.rs` — new dormant variants `CharacterOwnerMismatch` (and optionally `CharacterTokenExpired`) in the established `kind`-string house style.
- **Frontend (this change is NOT backend-only)**:
  - The user's own character list renders `token_status` — `token_expired` → "reconnect" affordance, `owner_mismatch` → "no longer on your account / remove" affordance. Exact copy deferred.
  - **A new admin Characters tab**: a server admin searches for a character by name, selects a result, and gets a dialog showing that character's **whole account** — every character on it with its `token_status` — so a transferred/expired character can be seen and acted on. The character listing supports surfacing/sorting/filtering by token state, at minimum to find `token_expired` (and `owner_mismatch`) characters together. (Backed by a new admin character-search endpoint reusing the `AdminAccount` extractor.)
  - Per `CLAUDE.md`, verification therefore includes the full frontend trio (`pnpm --filter frontend test` / `run check` / `run test:e2e`).
- **Out of scope (follow-up)**: automatic removal/cleanup of an `owner_mismatch` character's map/ACL entries (those tables don't exist yet); the design records the FK-cascade invariant they should follow. Proactive/SSE-driven refresh-while-active stays deferred — the daily sweep deliberately needs no notion of "active", so it has **no SSE dependency**.
- **Tooling**: schema/query changes require regenerating the sqlx offline cache (`cargo sqlx prepare -- --all-targets`) and committing the `.sqlx/` diff.
