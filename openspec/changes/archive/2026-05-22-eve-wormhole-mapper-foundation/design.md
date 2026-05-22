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
- Hard maximum session lifetime independent of activity (the foundation ships sliding 7-day idle expiry; "force re-login every 30 days regardless of activity" is a future capability)
- Automatic access-token refresh on expiry (both tokens are stored encrypted but the refresh-on-expiry flow is a future change)
- Owner-hash / character-transfer detection (intentionally omitted from this change; see Decision 3a)
- The 30-day soft-delete cooldown sweeper (a future scheduled-job change; this change only establishes the columns and reactivation behaviour)
- Map rendering, ACL UI, and `acls`/`maps` route content (the nav exposes `maps` and `characters` only in this change; `acls` is a later capability and the screenshots' `acls` link is intentionally omitted)
- Wormhole signature scanning, system info panels, sidebar / find-system input (the existing `zz-ref/frontend/wireframes/map_canvas.html` is a long-range visual target, not an in-scope deliverable)
- Frontend routing beyond `/`, `/login`, and `/characters`

## Decisions

### 1. Traefik v3 as reverse proxy (not nginx)

Traefik's Docker provider auto-discovers services via container labels, eliminating manual upstream configuration. It handles TLS termination cleanly via ACME for a future production deployment. The routing rules (`PathPrefix`) map directly to the `/auth/*` and `/api/*` → backend, everything else → frontend requirement.

*Alternative considered: nginx with static upstream config.* Rejected because it requires manual updates when service ports change and doesn't integrate as cleanly with Docker Compose.

### 2. Session cookie with server-side token storage

The browser holds only an opaque session ID inside an HS256-signed JWT (the cookie value). All OAuth2 tokens live in Postgres (`eve_character.encrypted_access_token` / `encrypted_refresh_token`), encrypted at rest with AES-256-GCM and keyed by `eve_character_id`, not by session. This eliminates token exposure to JavaScript and avoids PKCE complexity for a server-side flow.

*Alternative considered: storing tokens in a signed cookie.* Rejected — even encrypted, token material in the browser is higher risk and harder to revoke.

### 3. Postgres for identity, tokens, API keys, and sessions

A single durable store for everything authenticated:

- **`account` and `eve_character`** — identity and ESI tokens. Both the encrypted access token and encrypted refresh token live in `eve_character` columns. Identity survives restarts because losing it means re-onboarding every user; ESI tokens survive restarts so an active session that has just refreshed its access token doesn't lose it to a restart.
- **`api_key`** — long-lived bearer credentials issued under `/api/v1/keys`, stored as SHA-256 hashes. By definition holders expect them to keep working across deployments.
- **`session`** — session-cookie → account mapping with `csrf_state`, `add_character_mode`, and a sliding `expires_at`. Sessions persist across backend restarts so a user with a valid unexpired cookie stays logged in through a deploy; idle sessions auto-expire after 7 days. The session row holds NO token material — the cookie's JWT carries only `session_id`, and tokens stay in `eve_character`. See Decision 3d for the sliding-expiry mechanics and Decision 3e for the cookie-refresh behaviour.

In-flight OAuth2 records (the brief window between `/auth/login` redirect and `/auth/callback` return) remain in-memory: they have no `account_id` yet, are bound to one redirect cycle, and are intentionally restart-volatile (a backend restart mid-redirect just sends the user back through SSO). A sibling `InflightStore` (in-memory `HashMap`) holds these alongside the `SessionStore` (Postgres-backed).

`sqlx` is the database driver: compile-time-checked queries via `sqlx::query!`, native async with tokio.

*Alternative considered: SQLite.* Rejected because Postgres is the eventual production target for the domain data (maps, signatures, chain history) and there's no benefit to introducing SQLite only to migrate away from it later.

*Alternative considered: in-memory session map.* Rejected because every backend restart would silently 401 every authenticated browser. Will also break the moment we run more than one backend replica. Postgres is the smallest possible step that fixes both, with one extra DB write per authenticated request as the only cost.

*Alternative considered: Redis or `tower-sessions`.* Rejected — Redis adds an infra dependency for a session table that is already small and bounded by 7-day expiry; `tower-sessions` would replace the hand-rolled `SessionStore` but pulls in its own cookie shape and middleware without a clear win at this scale.

### 3d. Sliding 7-day session expiry

On session creation `expires_at = now() + interval '7 days'`. On every cookie-authenticated request the middleware runs a single `UPDATE session SET last_seen_at = now(), expires_at = now() + interval '7 days' WHERE session_id = $1 AND expires_at > now() RETURNING ...`. A row is treated as valid iff that `UPDATE` affected a row (i.e., the `WHERE` matched). This collapses "read + refresh" into one round-trip and atomically rejects already-expired rows.

API-key requests (`Authorization: Bearer erb_…`) bypass the session table entirely — they neither read nor extend any session row.

*Alternative considered: refresh only on a threshold (e.g., > 1 day since `last_seen_at`).* Saves a write on burst traffic but complicates reasoning. Postgres can comfortably absorb one UPDATE per authenticated request at our scale.

*Alternative considered: fixed lifetime with explicit refresh endpoint.* Standard OAuth pattern, but heavier for a single-page web UI and requires frontend changes.

Expired rows are physically removed opportunistically: a small fraction of authenticated requests issue `DELETE FROM session WHERE expires_at < now()`. The read path checks `expires_at > now()` anyway, so stale rows are inert in the meantime — a scheduled cleanup job is deferred until we have a job scheduler for something else.

### 3e. Session cookie JWT is refreshed on each authenticated request

When the middleware successfully refreshes a session row, the response also carries a fresh `Set-Cookie` header re-issuing the session JWT with `exp = now() + 7 days`. The session ID and signing key are unchanged; only `exp` advances. Without this, the browser-side cookie would expire on its original `exp` while the DB row was still good.

Implemented as a `tower` middleware layer that installs a request-scoped `RefreshedJwtSlot`; the `AuthenticatedAccount` extractor fills the slot when the session cookie was the auth source, and the layer writes the `Set-Cookie` header on the way out. API-key auth never fills the slot, so bearer responses never carry a refreshed cookie.

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
    corporation_name        TEXT        NOT NULL,
    alliance_id             BIGINT,
    alliance_name           TEXT,
    is_main                 BOOLEAN     NOT NULL DEFAULT false,
    is_online               BOOLEAN,
    esi_client_id           TEXT,
    encrypted_access_token  BYTEA,
    encrypted_refresh_token BYTEA,
    access_token_expires_at TIMESTAMPTZ,
    scopes                  TEXT[]      NOT NULL DEFAULT '{}',
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

CREATE TABLE session (
    session_id         TEXT        PRIMARY KEY,
    account_id         UUID        NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    csrf_state         TEXT,
    add_character_mode BOOL        NOT NULL DEFAULT FALSE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at         TIMESTAMPTZ NOT NULL
);

CREATE INDEX session_expires_at_idx ON session (expires_at);
CREATE INDEX session_account_id_idx ON session (account_id);
```

Notes on the columns and indexes:

- **`account.status`** is text. Initial values used by this change: `'active'` (default) and `'soft_deleted'`. Future statuses (`'banned'`, `'suspended'`, etc.) will be set account-wide via a per-character moderation action; the column is open-ended on purpose.
- **`account.delete_requested_at`** records when soft-delete was initiated. A future sweeper change will hard-delete accounts where `status = 'soft_deleted' AND delete_requested_at < now() - cooldown`. The cooldown is documented but not yet enforced (planned default: 30 days).
- **`account.is_server_admin`** flags accounts with administrative privileges across the deployment. The partial `account_server_admin_idx` makes "list all admins" a tiny, fast scan even when the table grows.
- **`eve_character.account_id` is NULLABLE.** An orphaned `eve_character` row is a public-info cache: it can be created by other flows (e.g. adding a character to a map ACL by name) before that pilot has ever signed in. When the pilot eventually logs in, the row is **claimed** by setting `account_id` to their account.
- **`eve_character.eve_character_id`** holds the BIGINT EVE character ID from ESI. `UNIQUE` so the same character can't be represented twice. The column is prefixed with `eve_` to distinguish it from `eve_character.id` (our internal UUID).
- **`corporation_id`** is `NOT NULL`; EVE characters always belong to a corporation. **`alliance_id`** is NULL when the corporation is not in an alliance.
- **`corporation_name` and `alliance_name`** are denormalised copies of the corresponding ESI public-info strings, written together with the IDs on every SSO callback (login, add-character, re-auth) and refreshed by a future background job that walks active accounts. They exist so `GET /api/v1/me` is a pure DB read — see §12. `corporation_name` follows `corporation_id` and is `NOT NULL`; `alliance_name` follows `alliance_id` and is NULL together with it.
- **`is_main`** marks one character per account as the primary identity. The partial unique index `eve_character_one_main_per_account` enforces "at most one main per account" while permitting `false`/NULL freely.
- **`is_online`** mirrors the last-known online state from the `esi-location.read_online.v1` scope; NULL until first poll.
- **`esi_client_id`** records which ESI client ID the stored tokens were issued under, so a future client-rotation can identify rows that need re-auth. NULL for orphan rows.
- **`encrypted_access_token` / `encrypted_refresh_token`** are AES-256-GCM ciphertexts (nonce prefixed inside the BYTEA; 12-byte nonce + ciphertext + auth tag). Both NULL for orphan rows. Persisting access tokens means a brief restart doesn't force a re-auth for users mid-session. `encrypted_refresh_token IS NOT NULL` is the canonical signal that the row is "live" — `token_status` in `/api/v1/me` derives from exactly this (see §12). Once a future refresh-on-demand flow is added, an ESI `invalid_grant` response SHALL NULL out both token columns, which flips `token_status` to `"expired"` mechanically without any additional state.
- **`access_token_expires_at`** records when the stored **access** token expires (~20 minutes after issue, per the EVE SSO contract). It exists so a future refresh-on-demand flow can decide whether to refresh before the next ESI call. It is NOT the refresh token's expiry; it does NOT drive `token_status`. The column was originally named `esi_token_expires_at`, which was ambiguous enough to mislead the design — see §12's "why `token_status` ignores this column" note.
- **`scopes`** is a `TEXT[]` of the ESI scope identifiers the user granted during SSO, parsed from the access-token JWT's `scp` claim. `NOT NULL DEFAULT '{}'`. Stored as a Postgres array so a future change that introduces required-scope-set drift detection (e.g. surfacing `token_status = "missing_scopes"` when the app upgrades to require a new scope) can do the subset check (`required_scopes <@ scopes`) without a JSON parse. No handler in this change reads the column; it exists so the future capability does not require a migration.
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

`session` columns:

- **`session_id`** is the opaque identifier carried inside the session-cookie JWT. The JWT's signature establishes integrity; this column is the lookup key.
- **`account_id`** is the authenticated account the session resolves to. `ON DELETE CASCADE` so account hard-deletion sweeps the account's sessions.
- **`csrf_state` / `add_character_mode`** are carried forward from the in-flight OAuth2 record when the session is created. Persisting them means a backend restart between SSO start and SSO callback does not strand the user.
- **`created_at`** records first sight; **`last_seen_at`** is advanced atomically with `expires_at` on every authenticated request (see Decision 3d); **`expires_at`** is the moment past which the row is treated as if it does not exist.
- **`session_expires_at_idx`** supports the opportunistic `DELETE FROM session WHERE expires_at < now()` cleanup; **`session_account_id_idx`** supports `list_session_ids_for_account` (used by `DELETE /api/v1/account` to drop every session belonging to a soft-deleted account).

### 3b. Account lifecycle (soft delete)

- Default state: `status = 'active'`, `delete_requested_at = NULL`.
- User requests deletion: backend sets `status = 'soft_deleted'`, `delete_requested_at = now()`. Active sessions for the account are dropped. Characters are NOT removed.
- Any subsequent SSO login as a character belonging to a `'soft_deleted'` account **reactivates** it: `status` returns to `'active'` and `delete_requested_at` is cleared, atomically with the token upsert.
- Hard delete after cooldown (planned 30 days) is **deferred to a future change**; once it runs, `ON DELETE CASCADE` cleans up the character rows.

### 3c. Character lifecycle

- **Created with an account** (normal login of a never-seen character): row inserted with `account_id` set.
- **Created without an account** (orphan, e.g. added to a map ACL by name): row inserted with `account_id = NULL`, `name` / `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name` populated from ESI public-info endpoints by the flow doing the insert. Tokens, `esi_client_id` remain NULL.
- **Claimed**: on first login for an orphan row's `eve_character_id`, set `account_id` to the logging-in user's account and write the tokens.
- **Re-login**: rewrite `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `scopes`, refresh `name` / `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name` from ESI, bump `updated_at`.
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

### 10. Visual design system — space-dark theme with JetBrains Mono

The authoritative visual contract for this change is the set of HTML wireframes (`login.html`, `home.html`, `characters.html`, `maps.html`, `user-menu.html`). They were authored under `openspec/changes/eve-wormhole-mapper-foundation/wireframes/` during the change's active lifetime and reviewed/approved *before* the frontend implementation tasks began; Svelte components SHALL match them. **Per §7.29, the wireframes were moved to `frontend/wireframes/` at archival time** — a tracked, durable location alongside the code they describe — so future frontend changes can reference and update them. The screenshots that previously lived in `zz-ref/frontend/screenshots/` were temporary aids for authoring the wireframes; they were deleted alongside the move since `zz-ref/` is gitignored and not a durable artefact location.

The long-range map wireframe at `zz-ref/frontend/wireframes/map_canvas.html` is the visual reference for future map work and the source of the shared design tokens below — it is not an in-scope deliverable for this change. (A future change SHOULD relocate it to `frontend/wireframes/map_canvas.html` so it is also durable.)

Key choices:

- **Typeface**: JetBrains Mono exclusively — reinforces the technical/terminal aesthetic appropriate for a scanning tool. Loaded from Google Fonts in `frontend/src/app.css`.
- **Colour palette**: deep navy `--space-950` → `--space-600` for backgrounds/surfaces; slate scale for text (`--slate-100` body, `--slate-300` secondary, `--slate-500` muted/disclaimer); `--sky` (`#38bdf8`) as the single brand accent (logo, active nav indicator, primary buttons, headings); `--emerald` for connected/positive states; `--amber` for warning; `--red` for destructive actions (`remove`, `delete account`).
- **Density**: 0.875rem (≈14px at the browser default) body font size; generous whitespace; 8px / 12px / 16px spacing rhythm. **All typography is sized in `rem`, not `px`**, so the entire interface scales when a visitor changes their browser font-size or zooms; spacing (paddings, margins, gaps, border-radii, avatar/icon dimensions) stays in `px` because those are visual-layout values that should not grow with text-size preferences. The `<html>` element is explicitly set to `font-size: 100%` so a future user-controllable text-size preference (see the in-flight `accessibility-preferences` change) can swap a single CSS custom property to scale the whole UI without touching component-level rules.
- **Card surfaces**: `background: var(--space-900)`, `border: 1px solid var(--space-700)`, `border-radius: 6px`, padding `16px–24px`.

The design system is implemented as CSS custom properties on `:root` in a global stylesheet (`frontend/src/app.css`), consumed by all Svelte components. No CSS framework — custom properties only. The token names are shared with `zz-ref/frontend/wireframes/map_canvas.html` so that future map work picks up the same palette without redefinition.

### 11. Visible UI surface in this change

The frontend in this change exposes four routes and one shared chrome element. Each is fully specified by its wireframe; the bullets below capture the load-bearing decisions that the wireframes cannot express.

**Routes:**

- `/login` — full-viewport `--space-950` background, no nav bar, centred card with the brand mark, "Wormhole Mapper" subtitle, EVE SSO login button (uses the official CCP-hosted PNG `https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png`), and a two-line disclaimer.
- `/` (home) — global nav at top; centred-left content area shows `Welcome, <main character name>` heading in `--sky` and the main character's name + `main` badge + corporation/alliance lines. No "Map view coming soon." line here — that placeholder lives on `/maps` so the two pages do not duplicate the same message. This **replaces** the originally-planned 288px collapsible sidebar + canvas placeholder; the sidebar belongs to the future map view (see `map_canvas.html`) and would be visual clutter against an empty canvas.
- `/maps` — global nav at top; centred body with a single line `Map view coming soon.` in `--slate-500`. Placeholder for the future map-rendering change. The `maps` nav link points here.
- `/characters` — global nav at top; page heading `CHARACTERS` in `--slate-500` uppercase, with a search box (top-right of the heading) and a `+ add character` button whose `href` is `/auth/characters/add?return_to=/characters`. Character cards are laid out in a **2-column responsive grid** (1 column on narrow viewports). Each card shows: 56–64px portrait from `https://images.evetech.net/characters/<eve_character_id>/portrait?size=128`; character name with a `main` badge (cyan pill) when applicable; faction-coloured corporation row; alliance row (omitted when null); and a footer row with a **token-status indicator** (small dot + `token active` in `--emerald` when the character's `token_status = "active"`, or `token expired` in `--red` when `token_status = "expired"`) and right-aligned action buttons. Non-main cards with an active token expose `set main` and `remove`; non-main cards with an expired token replace `set main` with `re-auth` in `--amber` (linking to `/auth/characters/add?return_to=/characters`, which re-runs the SSO callback and refreshes the stored tokens for the chosen character — see §12.5 below). The main character's card has no `set main` / `remove` actions but still shows its token-status indicator and a `re-auth` link when expired. The search box filters cards by **character name** (case-insensitive substring) entirely client-side; an empty state ("No characters match your search.") is shown when no card matches. Below the grid, a `DANGER ZONE` section with a single `delete account` text button in `--red`.

**Shared chrome:**

- **GlobalNav** (48px fixed bar, `--space-900`, bottom border `--space-700`):
  - Left: cyan sun logo SVG + `E-R BRIDGE` wordmark. Clicking links to `/`.
  - Centre-left: nav links `maps` (→ `/maps`), `characters` (→ `/characters`). Active link is indicated by `--sky` colour and a subtle `--space-700` background pill. The brand mark on the left links to `/` (home).
  - Right: pulsing `--emerald` status dot + `connected` label (driven by the success of `GET /api/v1/me`; the dot is `--red` and labelled `disconnected` if the request fails). To the right of the status, the user chip: 24px circular portrait of the **main** character + main character name + chevron, all clickable to toggle the user-menu dropdown.
- **User-menu dropdown** (opens beneath the user chip, anchored to its right edge, `--space-900` card with `--space-700` border):
  - `preferences` — disabled placeholder in this change (greyed-out, not a `<a>`). Future change.
  - `settings` — disabled placeholder in this change. Future change. The link target for both placeholders is `#`; they SHALL have `aria-disabled="true"` and `tabindex="-1"` and no hover effect.
  - Visual divider (`--space-700`).
  - `log out` — `<a href="/auth/logout">`.

The dropdown closes on outside-click and on `Escape`. State is local to the component (no store needed).

**Auth gating:**

`+layout.server.ts` runs on every navigation. It calls the backend's `GET /api/v1/me` server-side (forwarding the request's `cookie` header). On 401, it redirects to `/login` unless the requested route is already `/login`. On 200, it stashes the response in `event.locals` so child `+page.server.ts` files can reuse it without re-fetching. If the layout load reaches `/login` with an authenticated session, it redirects to `/`.

**Why this shape:**

The screenshots make the *structure* obvious but leave several behaviours ambiguous; the bullets above pin them down so the wireframes don't have to encode them. Specifically: which character drives the user chip portrait (the **main**, not the last-logged-in); `maps` resolves to a `/maps` placeholder page in this change (a real map view ships in a future change); and what happens when an account has only one character (it is automatically main; `remove` is hidden on the main card; `delete account` is the only way to remove the last identity).

### 12. Account-management endpoints under `/api/v1/`

To support the home and characters pages, four new `/api/v1/` endpoints are added (full contracts in `specs/account-management/spec.md`):

- `GET /api/v1/me` — returns the caller's account plus their full character list, including `corporation_name` / `alliance_name` read directly from the denormalised `eve_character` columns (no ESI calls in this handler), `portrait_url` constructed from the EVE image server, and a derived `token_status` enum per character (see below). Raw token columns are NEVER included in the response.
- `POST /api/v1/characters/:id/set-main` — promotes a character to main; flips `is_main` in a single transaction relying on the partial unique index `eve_character_one_main_per_account` for correctness.
- `DELETE /api/v1/characters/:id` — hard-deletes a non-main character. Rejects removal of the only character (409 `cannot_remove_last_character`) and removal of the current main while siblings exist (409 `cannot_remove_main`) — the caller must promote another character to main first.
- `DELETE /api/v1/account` — soft-deletes the caller's account (`status = 'soft_deleted'`, `delete_requested_at = now()`), deletes every row in `session` belonging to the account (logging out every browser the user is on), and clears the caller's session cookie. Character rows and API keys are NOT modified. A subsequent SSO login reactivates the account (per the `eve-sso-auth` capability).

**Per-character `token_status`.** Each element of `data.characters` SHALL carry a `token_status` field with the value `"active"` when the row's `encrypted_refresh_token IS NOT NULL`, and `"expired"` when it is `NULL`. Neither `access_token_expires_at` nor the `scopes` array SHALL appear on the wire. The field is a **string enum**, not a boolean, so future expansion (e.g. `"missing_scopes"` once required-scope-set drift detection lands, or a future `invalid_grant`-driven flip once refresh-on-demand exists) can land without a breaking change. The characters-page UI renders a `--emerald` / `--red` dot driven entirely by this value.

**Why `token_status` ignores `access_token_expires_at`.** An earlier draft of this spec defined `token_status` against the (then-named) `esi_token_expires_at > now()` rule. That rule was wrong for two reasons. First, the EVE access token has a ~20-minute lifetime; deriving `token_status` from it would flip every character to `"expired"` 20 minutes after every login, surfacing a `re-auth` prompt on a perfectly healthy session. Second, the column's meaning was ambiguous in the spec's own wording — one section said "access token expiry", another called it "the refresh token's durable expiry". The column has been renamed to `access_token_expires_at` for clarity and demoted to a private implementation detail of a future refresh-on-demand flow. `token_status` derives from `encrypted_refresh_token IS NULL` — the only piece of state that genuinely answers "can we still act on behalf of this character without re-doing SSO?" Until that future refresh flow lands, `token_status` is a strict upper bound on usability: it will not catch refresh tokens that ESI has silently revoked server-side. The foundation change does not make ESI calls on the user's behalf, so a revoked refresh token has no user-visible consequence here. See Risks/Trade-offs.

**Why this shape:**

- Splitting `GET /api/v1/me` from the existing `/auth/*` endpoints keeps the authenticated read of identity inside the versioned, envelope-wrapped `/api/*` surface. `/auth/*` is reserved for OAuth2 redirects and session-cookie management; it stays HTML-redirect-shaped, not JSON.
- `corporation_name` and `alliance_name` are **denormalised** onto `eve_character` and read directly from the DB on `GET /api/v1/me`. Resolving them from ESI per request was the original design but was rejected once it became clear `GET /api/v1/me` is called from the SvelteKit `+layout.server.ts` on every authenticated page load: an account with N linked characters would fan out to 2N serialised ESI calls per page, each carrying 100–500ms of latency, blocking TTFB. Names *can* change in EVE (corp rename, alliance reshuffle), but the staleness window is bounded by two write paths that both refresh the columns: every SSO callback (login, add-character, re-auth) and a future background job that polls active accounts via the SSE-based active-user set. The displayed value therefore lags real ESI state by at most one background-job interval for an active account, and refreshes immediately on the next login for an inactive one. That trade is acceptable; the prior design's per-request ESI fan-out was not.
- `portrait_url` is also resolved server-side, even though the URL is deterministic from `eve_character_id`, so that a future move to a different image host (or signed URLs) is a one-line change in the backend.
- `DELETE /api/v1/account` is the only place where the soft-delete state machine is *initiated* from the HTTP API; the reactivation path is the SSO callback (already specified). Keeping reactivation implicit (just log in again) means no UX prompt is needed, which matches Risks/Trade-offs §"Soft-delete reactivation is silent".
- The main-character invariant is enforced at two layers: the Postgres partial unique index `eve_character_one_main_per_account` (no two mains can coexist) and the `cannot_remove_main` / `cannot_remove_last_character` 409s (the API never leaves the account in an invalid state via deletion). The first linked character is automatically promoted to main during `upsert_character_from_login` when the account has no main yet.

### 12.5 Re-auth uses the existing `/auth/characters/add` flow

There is no dedicated re-auth endpoint. The characters-page `re-auth` link (shown on cards with `token_status = "expired"`) points at `/auth/characters/add?return_to=/characters` — the same redirect used by `+ add character`. The shared `/auth/callback` handler (§2.13) calls `characters::upsert_tokens` (§2.8), whose rule is: when the existing `eve_character` row's `account_id` matches the caller's resolved account, **overwrite tokens, refresh public info, and bump `updated_at`**. Re-auth is therefore mechanically identical to add: same redirect, same callback, same upsert. The button is a UX shortcut that pre-frames the action; the backend treats both flows uniformly.

**Caveat.** EVE SSO does not pre-select which character the user authorises — they pick at CCP's screen. If the user clicks `re-auth` on an expired card but authorises a different character, that other character's tokens are refreshed (or it is freshly added) and the original expired card remains expired. Acceptable: the user can simply click `re-auth` again and pick the right one. No backend change handles this asymmetry; it is documented here so frontend copy and tests don't assume targeted refresh.

### 13. OpenAPI doc via `utoipa`, strict response-validation test, frontend codegen deferred

The api-contract spec promises a "machine-readable description" of `/api/*`. This change discharges that promise with `utoipa` + `utoipa-swagger-ui`:

- Every `/api/v1/*` handler is annotated with `#[utoipa::path(...)]` declaring its request body, response shapes (one per status code), and security requirement.
- Every DTO (request bodies, response payloads, the success envelope, the error envelope) derives `utoipa::ToSchema`. The envelope types are declared once and referenced via `$ref` from every route response — envelope changes propagate without touching individual routes.
- A single `#[derive(OpenApi)]` collector references all annotated paths and component schemas; the resulting `utoipa::openapi::OpenApi` is served as `/api/openapi.json` and rendered by `utoipa-swagger-ui` at `/api/docs`.
- A backend test (`#[test] fn openapi_doc_matches_handler_responses`) instantiates the router with mocked services, hits each documented route with representative inputs, and validates the JSON response against the schema in the same `OpenApi` object. Drift between annotations and actual responses fails the build. This is the "strict" posture; the alternative ("descriptive, may drift") was rejected because the document only buys downstream tooling value if downstream tooling can trust it.

**Frontend codegen is explicitly deferred to a future change.** This change ships the backend OpenAPI doc and a hand-typed `frontend/src/lib/api.ts` whose types mirror it. Bundling a codegen pipeline (generator choice, build-step wiring, schema-drift detection, tsconfig integration) into the foundation change inflates scope without unlocking anything that hand-typing can't deliver while the API surface is this small. Once `account-management` and `api-authentication` stabilise, a follow-up change wires in `openapi-typescript` (or similar) and removes the hand-typed `api.ts` types.

*Alternatives considered:*

- **`axum-aide` / `okapi`.** Rejected — `utoipa` is the most actively maintained OpenAPI stack for Axum, has good `ToSchema` ergonomics, and ships a usable Swagger UI integration.
- **TypeSpec / hand-authored OpenAPI YAML.** Rejected — a hand-authored description is exactly the drift risk we're trying to avoid. Derive-from-code is non-negotiable.
- **Skip OpenAPI entirely, keep hand-typed frontend types.** Rejected — the api-contract spec already commits to a machine-readable description, and shipping one now is cheaper than retrofitting it later when more `/api/v1/*` routes exist.

### 14. UI surface for API errors

API errors surface in one of two places, depending on where they originate:

- **Form-action failures** (`POST /api/v1/characters/:id/set-main`, `DELETE /api/v1/characters/:id`, `DELETE /api/v1/account`, and the `/auth/characters/add` redirect chain) surface **inline**, anchored to the action that triggered them. A `set main` or `remove` failure shows a one-line error in `--red` directly below the character's card. A `delete account` failure shows a one-line error in `--red` directly below the DANGER ZONE button. The error text is the envelope's `error.message`; the `error.code` is exposed via `aria-describedby` (or a `data-error-code` attribute) so end-to-end tests can branch on the code without parsing message text.

- **Layout-level failures** (the `/api/v1/me` call in `+layout.server.ts`) surface as a **single top-of-page banner** in `--red` immediately under the GlobalNav, with text "Couldn't load your account: <message>". If the failure is 401, the layout instead redirects to `/login` (per §4.5) — no banner. If the failure is a network error or 5xx, the banner is shown and the page still renders whatever it can without `me` (the home page shows a generic "Welcome." with no name; the characters page shows an empty list; the user chip falls back to a generic placeholder icon).

**Not used:**

- **Toasts** — transient, easy to miss, and require a global store. Rejected: error feedback on a destructive action (`remove`, `delete account`) MUST be persistent until the user dismisses or retries.
- **`+error.svelte`** — SvelteKit's error route is for *unrecoverable* failures (the page cannot render at all). Our failures are recoverable: the user can retry, pick a different action, or just continue. Routing to `+error.svelte` for "you can't remove the main" would be a worse experience than an inline message.

**Why both, not just one:** form-action errors are local (a single button failed) and inline matches the user's attention. Layout-level errors are global (the whole page is degraded) and need a banner so the user understands why the page looks empty. Picking either alone leaves the other case awkward: a banner for "set main failed" feels disproportionate; an inline error for "couldn't load anything" has no good anchor.

## Risks / Trade-offs

- **One extra DB write per cookie-authenticated request** → The middleware `UPDATE`s the session row to advance `last_seen_at` and `expires_at` on every request. Acceptable at this scale; the same request already touches `account` indirectly. Bearer-token (API-key) requests bypass this entirely.
- **JWT replay window grows from "until original `exp`" to "until DB row expires"** → Intentional. This is the whole point of the persisted-session design — the cookie's `exp` is advanced on every authenticated request so it tracks the server-side row, not the original issue time. The session ID is still server-revocable via `delete` (logout / soft-delete / future "log out everywhere" admin endpoint).
- **Opportunistic session cleanup leaks rows under sustained read-only traffic** → A small fraction of authenticated requests issue `DELETE FROM session WHERE expires_at < now()`. Under any realistic load the table stays small (rows are ≤ 7 days old). Worst case is migrating to a scheduled task later when a job scheduler exists for something else (ESI refresh, character refresh).
- **Tokens at rest in Postgres** → Both access and refresh tokens persist encrypted (AES-256-GCM). DB compromise + `ENCRYPTION_SECRET` compromise = token compromise; that's the threat model, and it's the same as any encrypted-at-rest scheme. The application MUST never log decrypted tokens.
- **Migrations run at startup** → On boot the backend applies any new migrations before serving traffic. A broken migration prevents startup, which is the correct failure mode for a single-instance deployment. Multi-instance deployments would need to gate migrations separately.
- **`ENCRYPTION_SECRET` rotation requires re-encrypting tokens** → Documented; out of scope to implement an automatic rotation tool. Manual rotation procedure deferred to a future ops-focused change.
- **No automatic token refresh, and `token_status` is therefore an *upper bound* on usability** → This change stores both access and refresh tokens encrypted, parses and persists the granted ESI `scopes`, but does not yet use the refresh token to mint a new access token on demand. The refresh-on-demand flow is the next auth capability; it is deliberately deferred because the foundation makes no ESI calls on the user's behalf (the only ESI calls in this change happen during the SSO callback itself, where we already hold a fresh access token). Consequently `token_status` derives only from `encrypted_refresh_token IS NULL` — i.e. "we hold a refresh token that we believe still works." It will read `"active"` for a row whose refresh token has been server-side revoked by CCP (account suspended, credentials rotated, user revoked ESI access in their EVE settings) until either (a) a future refresh-on-demand call to ESI returns `invalid_grant` and NULLs the tokens, or (b) the user re-runs SSO. For the foundation's UX surface — `/`, `/maps`, `/characters` — that gap is invisible: nothing on those pages exercises a refresh. The `re-auth` link on `/characters` is therefore largely decorative in this change; it is wired up because the *future* capability that does exercise refresh will need it, and shipping the UI shape now means the future change is purely a backend wiring task.
- **No character-transfer detection** → Without `owner_hash`, if an EVE character is transferred between EVE accounts, the new owner can log in and silently take over the existing row. Accepted for this change; revisit if/when ESI characters become tradeable at scale matters.
- **Denormalised corp/alliance names can go stale** → `corporation_name` and `alliance_name` are stored on `eve_character` and refreshed only on SSO callbacks and (in a future change) by a background job over active accounts. A corp or alliance rename in EVE will not appear in the UI until one of those write paths runs for the affected row. Acceptable: the bound on staleness for active users is the background-job interval; for inactive users it is "until next login". The alternative — fetching from ESI on every `GET /api/v1/me` — was rejected because `/api/v1/me` is called from the SvelteKit root `+layout.server.ts` on every authenticated page load, which would serialise 2N ESI round-trips into every page's TTFB.
- **Disabled `preferences` / `settings` menu items** → Greyed-out placeholders ship in the user-menu dropdown to lock in the visual layout from the screenshots, but they have no destinations. A user clicking them gets no feedback. Acceptable as a deliberate placeholder; the alternative (hiding them) would mean the menu has a single `log out` row and looks empty.
- **`utoipa` annotations are a maintenance surface** → Every new handler and DTO needs `#[utoipa::path]` / `#[derive(ToSchema)]`. The strict response-validation test catches drift but not omission (a handler with no annotation is just missing from the doc). Acceptable — the `rust-rest-api` skill mandates annotations, and the doc-coverage scenario in api-contract requires every `/api/v1/*` route to appear in the doc, which the integration check exercises.
- **`return_to` validation must be tight** → Same-origin path validation is a known open-redirect surface. The spec explicitly rejects scheme-relative (`//evil.com`) and absolute URLs and limits the value to a path starting with a single `/`. Any future relaxation (e.g. supporting fragment identifiers in the path) MUST be reviewed against the open-redirect risk.
- **Frontend types are hand-maintained for now** → `frontend/src/lib/api.ts` duplicates the response shapes that the backend OpenAPI doc already describes. Drift is possible (a backend change adds a field; the frontend type lags). Mitigated by the small API surface in this change and the explicit follow-up change to introduce codegen. Until then, reviewers MUST check both sides when a `/api/v1/*` route changes.
- **Soft-delete reactivation is silent** → A user whose account is `soft_deleted` will be reactivated on next login with no UI prompt. Acceptable because soft-delete is user-initiated; if it was admin-initiated (banned/suspended) that'll be a different status and login will be refused.
- **API keys cannot be retrieved after creation** → If a user loses the plaintext, the only recovery is revoke + create-new. Documented in the create response. Stronger UX than storing recoverable plaintext.
- **No `last_used_at` on API keys** → Users can't tell whether a key is in use before revoking it. Accepted to keep the schema lean; trivial to add later (single-column migration with `NULL` default).
- **Server-scoped API keys have no authority yet** → The column exists for future use; no route currently honours them. A `server`-scoped key is effectively inert until a later change defines its permissions.
- **Single Traefik instance, no HA** → This is a single-user/small-team tool; HA is not required.
- **`SameSite=Lax` without `Secure` in dev** → Cookie is transmitted over HTTP in local development. This is intentional and documented. Production deployment MUST add `Secure`.
