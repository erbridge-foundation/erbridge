## Context

Greenfield EVE Online wormhole mapping tool. The backend must authenticate EVE pilots via EVE ESI OAuth2 (SSO), manage multi-character sessions, and expose a REST API for future wormhole chain features. The frontend is a thin SvelteKit SPA served by a Node adapter. Traefik routes requests so that a single public port reaches either service based on path prefix.

There are no existing services or databases to migrate. Identity data (accounts and linked EVE characters) and ESI tokens (both access and refresh, encrypted at rest with AES-256-GCM) are persisted to Postgres from day one — these are durable records that must survive backend restarts. Ephemeral session state (active session ID → account ID mapping, CSRF state for in-flight OAuth2 redirects) remains in process memory; sessions are intentionally invalidated on restart and users re-login. The character table also supports **orphan** rows with `account_id = NULL`, populated as a public-info cache by flows like ACL pre-claim — these are claimed when the pilot eventually signs in.

## Goals / Non-Goals

**Goals:**
- Single `docker compose up --build` starts the entire stack
- EVE SSO OAuth2 flow works end-to-end: login → callback → session cookie → frontend reads session
- Multiple characters can be linked to one session via add-character flow
- Session tokens never leave the backend; the browser only holds a signed, encrypted session ID cookie
- ESI discovery document is fetched once at startup and cached — no hardcoded endpoint URLs
- Idiomatic Rust: `thiserror` error types, no `unwrap` in production paths, minimal cloning

**Non-Goals:**
- Wormhole chain visualization or signature scanning
- Domain tables beyond `account` and `eve_character` (maps, systems, signatures come in later changes)
- Persistent sessions across restarts (sessions remain in-memory; users re-login after a backend restart)
- Automatic access-token refresh on expiry (both tokens are stored encrypted but the refresh-on-expiry flow is a future change)
- Owner-hash / character-transfer detection (intentionally omitted from this change; see Decision 3a)
- The 30-day soft-delete cooldown sweeper (a future scheduled-job change; this change only establishes the columns and reactivation behaviour)
- Frontend routing beyond `/` and `/login`

## Decisions

### 1. Traefik v3 as reverse proxy (not nginx)

Traefik's Docker provider auto-discovers services via container labels, eliminating manual upstream configuration. It handles TLS termination cleanly via ACME for a future production deployment. The routing rules (`PathPrefix`) map directly to the `/auth/*` and `/api/*` → backend, everything else → frontend requirement.

*Alternative considered: nginx with static upstream config.* Rejected because it requires manual updates when service ports change and doesn't integrate as cleanly with Docker Compose.

### 2. Session cookie with server-side token storage

The browser holds only an opaque session ID (AES-256-GCM encrypted, HS256 JWT signed). All OAuth2 tokens live in backend memory keyed by session ID. This eliminates token exposure to JavaScript and avoids PKCE complexity for a server-side flow.

*Alternative considered: storing tokens in a signed cookie.* Rejected — even encrypted, token material in the browser is higher risk and harder to revoke.

### 3. Postgres for identity + tokens, in-memory map for session routing

Two distinct stores, chosen for their distinct durability needs:

- **`account` and `eve_character` tables in Postgres** — identity AND ESI tokens are durable. Both the encrypted access token and encrypted refresh token live in `eve_character` columns. Postgres is the single source of truth for tokens; any login flow writes both. Identity survives restarts because losing it means re-onboarding every user; tokens survive restarts so an active session that has just refreshed its access token doesn't lose it to a restart.
- **`HashMap<SessionId, Session>` behind `Arc<RwLock<>>`** — sessions are ephemeral routing. Each session entry holds only `account_id` and CSRF state for in-flight OAuth2 redirects. Losing these on restart is acceptable; users re-login. There are no tokens in this map.

This split keeps the session table out of Postgres (no expiry-sweep job needed) while making tokens durable. `sqlx` is the database driver: compile-time-checked queries via `sqlx::query!`, native async with tokio.

*Alternative considered: SQLite.* Rejected because Postgres is the eventual production target for the domain data (maps, signatures, chain history) and there's no benefit to introducing SQLite only to migrate away from it later.

*Alternative considered: persisting sessions in Postgres too.* Rejected for this change — adds schema, expiry logic, and DB load for data that is intentionally short-lived. Can be revisited if multi-instance backends are needed.

### 3a. Schema

All table names are **singular** by project convention. The schema below is the authoritative version for this change.

```sql
CREATE TABLE account (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    status              TEXT        NOT NULL DEFAULT 'active',
    delete_requested_at TIMESTAMPTZ,
    is_server_admin     BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX account_server_admin_idx ON account (id) WHERE is_server_admin = TRUE;

CREATE TABLE eve_character (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id              UUID        REFERENCES account(id) ON DELETE CASCADE,
    eve_character_id        BIGINT      NOT NULL UNIQUE,
    name                    TEXT        NOT NULL,
    corporation_id          BIGINT      NOT NULL,
    alliance_id             BIGINT,
    is_main                 BOOLEAN     NOT NULL DEFAULT false,
    is_online               BOOLEAN,
    esi_client_id           TEXT,
    encrypted_access_token  BYTEA,
    encrypted_refresh_token BYTEA,
    esi_token_expires_at    TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX eve_character_one_main_per_account
    ON eve_character(account_id)
    WHERE is_main = true;

CREATE INDEX eve_character_account_id_idx ON eve_character (account_id);

CREATE TABLE api_key (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    scope         TEXT        NOT NULL,
    account_id    UUID        REFERENCES account(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    key_hash      TEXT        NOT NULL,
    expires_at    TIMESTAMPTZ NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT api_key_scope_check CHECK (
        (scope = 'account' AND account_id IS NOT NULL)
        OR (scope = 'server' AND account_id IS NULL)
    )
);

CREATE UNIQUE INDEX api_key_hash_idx ON api_key (key_hash);
CREATE INDEX api_key_account_idx ON api_key (account_id) WHERE account_id IS NOT NULL;
```

Notes on the columns and indexes:

- **`account.status`** is text. Initial values used by this change: `'active'` (default) and `'soft_deleted'`. Future statuses (`'banned'`, `'suspended'`, etc.) will be set account-wide via a per-character moderation action; the column is open-ended on purpose.
- **`account.delete_requested_at`** records when soft-delete was initiated. A future sweeper change will hard-delete accounts where `status = 'soft_deleted' AND delete_requested_at < now() - cooldown`. The cooldown is documented but not yet enforced (planned default: 30 days).
- **`account.is_server_admin`** flags accounts with administrative privileges across the deployment. The partial `account_server_admin_idx` makes "list all admins" a tiny, fast scan even when the table grows.
- **`eve_character.account_id` is NULLABLE.** An orphaned `eve_character` row is a public-info cache: it can be created by other flows (e.g. adding a character to a map ACL by name) before that pilot has ever signed in. When the pilot eventually logs in, the row is **claimed** by setting `account_id` to their account.
- **`eve_character.eve_character_id`** holds the BIGINT EVE character ID from ESI. `UNIQUE` so the same character can't be represented twice. The column is prefixed with `eve_` to distinguish it from `eve_character.id` (our internal UUID).
- **`corporation_id`** is `NOT NULL`; EVE characters always belong to a corporation. **`alliance_id`** is NULL when the corporation is not in an alliance.
- **`is_main`** marks one character per account as the primary identity. The partial unique index `eve_character_one_main_per_account` enforces "at most one main per account" while permitting `false`/NULL freely.
- **`is_online`** mirrors the last-known online state from the `esi-location.read_online.v1` scope; NULL until first poll.
- **`esi_client_id`** records which ESI client ID the stored tokens were issued under, so a future client-rotation can identify rows that need re-auth. NULL for orphan rows.
- **`encrypted_access_token` / `encrypted_refresh_token`** are AES-256-GCM ciphertexts (nonce prefixed inside the BYTEA; 12-byte nonce + ciphertext + auth tag). Both NULL for orphan rows. Persisting access tokens means a brief restart doesn't force a re-auth for users mid-session.
- **`esi_token_expires_at`** records when the stored access token expires so we can decide whether to refresh before the next ESI call.
- **No `owner_hash`, no `scopes` column.** Owner-hash transfer detection is intentionally not implemented in this change — the simpler model is that a re-login just overwrites the stored tokens. Scopes granted are static per build (defined by the auth code), so storing them per row would be redundant.

`api_key` columns:

- **`scope`** is `'account'` or `'server'`. The CHECK constraint guarantees `account_id` is NOT NULL for `'account'` keys and NULL for `'server'` keys. Future scopes (e.g. `'acl'`) will land via a migration that loosens the CHECK.
- **`account_id`** uses `ON DELETE CASCADE` so account hard-deletion sweeps an account's keys.
- **`name`** is a user-chosen label so the owner can recognise their keys ("ci pipeline", "my laptop", …). Not unique; collisions are the user's problem.
- **`key_hash`** is `SHA-256(plaintext_key)` stored as a lowercase hex string. Lookup-by-hash is a single index probe — fast because the underlying token is high-entropy random and doesn't need a slow KDF.
- **`expires_at`** NULL means "no expiry"; otherwise the key MUST be rejected after this instant.
- **No `revoked_at`.** Revocation = `DELETE FROM api_key WHERE id = ...`. Simpler than a tombstone state; we don't need audit (yet).
- **No `last_used_at`.** Deferred; adding it later is a single-column migration.
- **`api_key_hash_idx`** is `UNIQUE` — collision on `key_hash` would be a SHA-256 collision, which we treat as impossible, and the unique constraint is a safety net.
- **`api_key_account_idx`** is partial (`WHERE account_id IS NOT NULL`) so "list keys for this account" is a tight index scan and `server`-scoped rows don't bloat it.

### 3b. Account lifecycle (soft delete)

- Default state: `status = 'active'`, `delete_requested_at = NULL`.
- User requests deletion: backend sets `status = 'soft_deleted'`, `delete_requested_at = now()`. Active sessions for the account are dropped. Characters are NOT removed.
- Any subsequent SSO login as a character belonging to a `'soft_deleted'` account **reactivates** it: `status` returns to `'active'` and `delete_requested_at` is cleared, atomically with the token upsert.
- Hard delete after cooldown (planned 30 days) is **deferred to a future change**; once it runs, `ON DELETE CASCADE` cleans up the character rows.

### 3c. Character lifecycle

- **Created with an account** (normal login of a never-seen character): row inserted with `account_id` set.
- **Created without an account** (orphan, e.g. added to a map ACL by name): row inserted with `account_id = NULL`, populated from ESI public-info endpoints. Tokens, `esi_client_id` remain NULL.
- **Claimed**: on first login for an orphan row's `eve_character_id`, set `account_id` to the logging-in user's account and write the tokens.
- **Re-login**: rewrite `encrypted_access_token`, `encrypted_refresh_token`, `esi_token_expires_at`, refresh `name` / `corporation_id` / `alliance_id` from ESI, bump `updated_at`.
- **Removed from account**: the row is hard-deleted (`DELETE FROM eve_character WHERE id = ...`). There is no soft-delete for characters; a removed character can be re-added later, which will create a fresh row.
- **Account hard-delete** (future sweeper): `ON DELETE CASCADE` removes all the account's character rows, including orphans the account had claimed.

### 3d. API key lifecycle

- **Issued** via `POST /api/v1/keys` by a session-authenticated request. The backend generates a key, hashes it, inserts the row, and returns the plaintext **once** in the response. The plaintext is never persisted in any form server-side.
- **Used** as `Authorization: Bearer <key>` on any `/api/*` request. The auth middleware hashes the presented value and looks up `key_hash`. If found and not expired, the request is authenticated as the key's owning account (for `'account'` scope) — same permissions as a session for that account would have.
- **Listed** via `GET /api/v1/keys`: returns metadata only (no `key_hash`, no plaintext).
- **Revoked** via `DELETE /api/v1/keys/:id`: hard-deletes the row. The key is rejected on the very next request.
- **Expired**: requests presenting an expired key are rejected with HTTP 401. Expired rows are not auto-deleted; a cleanup sweeper is a future concern.
- **Server scope**: keys with `scope = 'server'` exist in the schema but no `/api/*` route currently grants them any authority. They are creatable only through direct DB / admin tooling (not via `/api/v1/keys` in this change). Their authorization semantics will be defined in a later change.

### 3e. Migrations

`sqlx::migrate!("./backend/migrations")` is invoked at startup. Migrations are plain SQL files in `backend/migrations/` named with a timestamp prefix (`20260516120000_create_account_and_eve_character.sql`). The first migration enables `pgcrypto` (for `gen_random_uuid()`) and creates the three tables and all indexes above.

### 4. ESI SSO discovery at startup, cached in AppState

`reqwest::Client` fetches the well-known document once; `EsiMetadata` is stored directly in `AppState` (cloneable via `Arc`). This avoids repeated HTTP calls on every auth request and satisfies the "no hardcoded endpoint URLs" constraint.

### 5. SvelteKit with `@sveltejs/adapter-node`

The node adapter builds the frontend to a standalone Node.js server, which runs in its own Docker container. This is consistent with how the rest of the stack is containerized and avoids needing a separate static file server.

### 6. Cookie attributes: `httpOnly`, `SameSite=Lax`, `Path=/`

`httpOnly` prevents JavaScript from reading the session ID. `SameSite=Lax` allows the cookie to be sent on top-level navigation (needed for the OAuth2 redirect back to `/`) while blocking cross-site sub-resource requests. `Secure` is omitted for local development but SHOULD be set in production via configuration.

### 7. Derived keys from single `ENCRYPTION_SECRET`

A single `ENCRYPTION_SECRET` (32 hex bytes, 256 bits) is the root secret. Three keys derive from it (or it is used directly, documented in code):

1. **AES-256-GCM key for ESI tokens at rest** — used when writing/reading both `encrypted_access_token` and `encrypted_refresh_token` in `eve_character`
2. **AES-256-GCM key for the session cookie payload** — wraps the opaque session ID
3. **HS256 key for the session cookie JWT signature**

A single root secret simplifies rotation. Rotating `ENCRYPTION_SECRET` invalidates all sessions (acceptable) and renders existing stored tokens unreadable, so a rotation runbook would need to re-encrypt the `encrypted_access_token` and `encrypted_refresh_token` columns under the new key — documented as a future operational concern, not implemented here.

### 8. License

The app will be licensed under AGPL-3.0.

### 9. API key format and authentication

**Format.** API keys have the structure:

```
erb_<43 chars base64url>
```

- `erb_` is a fixed prefix. It aids leak detection — secret scanners (GitHub, gitleaks) can pattern-match it, and it tags the key as belonging to this app in pasted logs.
- The body is 32 random bytes (256 bits of entropy, from a CSPRNG) encoded as unpadded base64url. That yields exactly 43 characters using `[A-Za-z0-9_-]`, so the full key is **47 characters**, header-safe and URL-safe.
- 256 bits is overwhelming entropy for a random token; a slow KDF (Argon2 etc.) buys nothing. We use plain SHA-256 of the entire key (prefix + body), stored as lowercase hex in `key_hash`. Lookup is `SELECT ... WHERE key_hash = $1` — one index probe.

**Authentication.** On `/api/*` requests the auth middleware does, in order:

1. Look for `Authorization: Bearer <value>`. If present and `<value>` starts with `erb_`:
   - Compute `SHA-256(value)` as hex.
   - `SELECT * FROM api_key WHERE key_hash = $1 AND (expires_at IS NULL OR expires_at > now())`.
   - On hit with `scope = 'account'`, the request is authenticated as `account_id`.
   - On hit with `scope = 'server'`, the request is authenticated as a server-scoped caller (no `account_id`); routes in this change reject server-scoped callers since no route currently grants them permission.
   - Miss / expired → HTTP 401.
2. Otherwise fall back to the session cookie used by `/auth/*`. If neither auth method succeeds → HTTP 401.

**No fallbacks.** Query-string keys (`?api_key=...`) are deliberately not supported. Keys must not appear in request URLs because URLs end up in access logs, browser history, and referrers.

**Plaintext exposure.** The plaintext key is returned to the client exactly once — in the JSON response body of `POST /api/v1/keys`. It is never stored, never re-displayed, never logged. If the user loses it, they revoke and create a new one.

*Alternatives considered:*
- **Argon2id on the hash.** Rejected — adds CPU per request for no security gain when the token is already 256-bit random.
- **Hex encoding.** Rejected — base64url is 25% shorter and equally URL-safe.
- **No prefix.** Rejected — the prefix is essentially free and makes operational secret-scanning far more reliable.

### 9. Visual design system — space-dark theme with JetBrains Mono

The wireframe (`zz-ref/frontend/wireframes/map_canvas.html`) defines the complete design language. Key choices:

- **Typeface**: JetBrains Mono exclusively — reinforces the technical/terminal aesthetic appropriate for a scanning tool
- **Colour palette**: deep navy `--space-950` → `--space-600` for backgrounds/surfaces; slate scale for text; `--sky` (`#38bdf8`) as the single brand accent (logo, active tab indicator, primary buttons); `--emerald` for online/positive; `--amber` for warning; `--red` for destructive
- **Global nav**: 48px fixed bar, `--space-900` surface, brand logo + wordmark left, nav links centre-left, status dot + find input + logout icon right
- **Left sidebar**: 288px collapsible panel with a 40px icon-rail collapsed state; toggle button overflows the sidebar edge; sections use uppercase 10px headers with chevrons; state persists to `localStorage`
- **Login page**: same shell (nav bar + `--space-950` background); centred CTA; `--sky` accent on the login button

The design system is implemented as CSS custom properties on `:root` in a global stylesheet, consumed by all Svelte components. No CSS framework — custom properties only.

## Risks / Trade-offs

- **In-memory sessions lost on restart** → Acceptable for this change; durable identity and tokens are in Postgres, so users re-login but their account, linked characters, and last-stored ESI tokens remain.
- **Tokens at rest in Postgres** → Both access and refresh tokens persist encrypted (AES-256-GCM). DB compromise + `ENCRYPTION_SECRET` compromise = token compromise; that's the threat model, and it's the same as any encrypted-at-rest scheme. The application MUST never log decrypted tokens.
- **Migrations run at startup** → On boot the backend applies any new migrations before serving traffic. A broken migration prevents startup, which is the correct failure mode for a single-instance deployment. Multi-instance deployments would need to gate migrations separately.
- **`ENCRYPTION_SECRET` rotation requires re-encrypting tokens** → Documented; out of scope to implement an automatic rotation tool. Manual rotation procedure deferred to a future ops-focused change.
- **No automatic access-token refresh** → ESI access tokens expire after ~20 minutes. Refresh tokens are stored but not yet used to refresh on demand. Users will need to re-authenticate when their access token expires. Refresh-on-expiry is the next auth improvement and is unblocked by tokens now being in Postgres.
- **No character-transfer detection** → Without `owner_hash`, if an EVE character is transferred between EVE accounts, the new owner can log in and silently take over the existing row. Accepted for this change; revisit if/when ESI characters become tradeable at scale matters.
- **Soft-delete reactivation is silent** → A user whose account is `soft_deleted` will be reactivated on next login with no UI prompt. Acceptable because soft-delete is user-initiated; if it was admin-initiated (banned/suspended) that'll be a different status and login will be refused.
- **API keys cannot be retrieved after creation** → If a user loses the plaintext, the only recovery is revoke + create-new. Documented in the create response. Stronger UX than storing recoverable plaintext.
- **No `last_used_at` on API keys** → Users can't tell whether a key is in use before revoking it. Accepted to keep the schema lean; trivial to add later (single-column migration with `NULL` default).
- **Server-scoped API keys have no authority yet** → The column exists for future use; no route currently honours them. A `server`-scoped key is effectively inert until a later change defines its permissions.
- **Single Traefik instance, no HA** → This is a single-user/small-team tool; HA is not required.
- **`SameSite=Lax` without `Secure` in dev** → Cookie is transmitted over HTTP in local development. This is intentional and documented. Production deployment MUST add `Secure`.
- **`Arc<RwLock<>>` write contention under load on the session map** → Not a concern for expected usage (single pilot / small corp).
