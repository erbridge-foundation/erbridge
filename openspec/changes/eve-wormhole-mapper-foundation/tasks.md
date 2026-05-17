## 0. Required skills

Before implementing tasks, the implementer MUST invoke the skill matching the area being worked on. Each skill defines mandatory architecture, structure, and convention rules that this change relies on. Invoke via the `Skill` tool.

| Working on… | Skill | Invoke when |
|---|---|---|
| Anything under `backend/` (sections 2, 2b, 2c, parts of 6) | `rust-rest-api` | Before writing the first line of Rust in this session |
| Anything under `frontend/` (sections 4a, 4, 5) | `sveltekit-node` | Before writing the first line of Svelte / TypeScript in `frontend/` in this session. §4a wireframes are plain HTML and do NOT require this skill, but they must be approved before §4 begins. |

If you (Claude) reach a backend or frontend task and the relevant skill body has not been loaded in this session, stop and invoke it first. Loading both up-front is fine; they are independent.

## 1. Repository Scaffold

- [ ] 1.1 Create root-level `frontend/` and `backend/` directories
- [ ] 1.2 Write `.env.example` with `APP_URL`, `ENCRYPTION_SECRET`, `ESI_CLIENT_ID`, `ESI_CLIENT_SECRET`, `DATABASE_URL`, `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` — no secret defaults, comments for each
- [ ] 1.3 Add top level `.gitignore` file covering `.env`, `.idea/`, `.vscode/`, `.DS_Store`, `*Zone.Identifier`, `zz-ref/`
- [ ] 1.3 Add component level `.gitignore` files
- [ ] 1.3.1 Backend using standard Rust .gitignore from `https://github.com/github/gitignore/blob/main/Rust.gitignore`
- [ ] 1.4.2 Frontend using the following .gitignore verbatim
```
# Playwright
playwright-report/

node_modules
dist
test-results/
package-lock.json
yarn.lock
vite.config.js.timestamp-*
/packages/package/test/**/package
/documentation/types.js
.vercel_build_output
.svelte-kit
.cloudflare
.pnpm-debug.log
.netlify
.turbo
.vercel
.test-tmp
symlink-from
_tmp_flaky_test_output.txt
```

## 2. Backend: Rust/Axum Project

- [ ] 2.1 Initialise Cargo project in `backend/` (`cargo init`)
- [ ] 2.2 Add dependencies to `Cargo.toml`: `axum`, `tokio` (full), `reqwest` (json feature), `serde`/`serde_json`, `thiserror`, `anyhow`, `aes-gcm`, `jsonwebtoken`, `uuid` (v7 + serde features), `tower-http` (cors/trace), `dotenvy`, `sqlx` (postgres + runtime-tokio-rustls + uuid + chrono + macros features), `chrono` (serde feature)
- [ ] 2.3 Implement `backend/src/esi/mod.rs` with `EsiMetadata` struct and `discover()` function verbatim as specified
- [ ] 2.4 Implement `backend/src/config.rs`: read `APP_URL`, `ENCRYPTION_SECRET`, `ESI_CLIENT_ID`, `ESI_CLIENT_SECRET`, `DATABASE_URL` from env; fail fast with clear error if any are missing
- [ ] 2.5 Create `backend/migrations/00000000000001_create_account_eve_character_and_api_key.sql`: `CREATE EXTENSION IF NOT EXISTS pgcrypto;` then `CREATE TABLE account (...)`, `CREATE TABLE eve_character (...)`, and `CREATE TABLE api_key (...)` matching the schema in design.md §3a verbatim, including all indexes (`account_server_admin_idx`, `eve_character_one_main_per_account`, `eve_character_account_id_idx`, `api_key_hash_idx`, `api_key_account_idx`). Table names MUST be singular.
- [ ] 2.6 Implement `backend/src/db/mod.rs`: `connect(database_url: &str) -> Result<PgPool>` that creates a pool with a bounded initial-connection retry, then runs `sqlx::migrate!("./migrations").run(&pool).await`
- [ ] 2.7 Implement `backend/src/db/accounts.rs`:
  - `create_account() -> Result<Uuid>` — inserts a row with defaults, returns `id`
  - `get_account(id) -> Result<Option<Account>>` — returns the row including `status`, `delete_requested_at`, `is_server_admin`
  - `reactivate_if_soft_deleted(tx, id)` — sets `status = 'active'`, `delete_requested_at = NULL` only when `status = 'soft_deleted'`; takes a transaction so it can be atomic with character upsert
  - `soft_delete(id)` — sets `status = 'soft_deleted'`, `delete_requested_at = now()`
- [ ] 2.8 Implement `backend/src/db/characters.rs`:
  - `upsert_character_from_login(tx, account_id, eve_character_id, name, corporation_id, alliance_id, esi_client_id, access_token_plaintext, refresh_token_plaintext, expires_at)` — encrypts both tokens with fresh nonces, then performs `INSERT ... ON CONFLICT (eve_character_id) DO UPDATE` with the rule: if existing `account_id IS NULL` (orphan claim) OR matches the supplied one, set `account_id = excluded.account_id` and rewrite tokens / public info; otherwise leave `account_id` unchanged but still update public info and tokens (re-login on owned row). Bumps `updated_at`.
  - `create_orphan(eve_character_id, name, corporation_id, alliance_id)` — inserts a row with `account_id = NULL` and NULL token columns
  - `list_for_account(account_id)` — returns characters (no decrypted tokens)
  - `delete_character(id)` — hard `DELETE`
  - `set_main(tx, account_id, character_id)` — in one transaction, clears existing `is_main` on the account then sets it on the target
- [ ] 2.9 Implement `backend/src/session.rs`: `Session` struct (`session_id: String`, `account_id: Uuid`, `csrf_state: Option<String>`, `add_character_mode: bool`); `SessionStore` (`Arc<RwLock<HashMap<String, Session>>>`); add/remove/get helpers. Sessions do NOT hold token material — tokens live in Postgres only.
- [ ] 2.10 Implement `backend/src/auth/crypto.rs`: AES-256-GCM encrypt/decrypt helpers for ESI tokens at rest (`encrypt_token` returns `nonce || ciphertext || tag` packed BYTEA; `decrypt_token` inverse) and for the session cookie payload; HS256 sign/verify for session cookie JWT; all keyed from `ENCRYPTION_SECRET`
- [ ] 2.11 Implement `backend/src/auth/cookie.rs`: helpers to create and clear the `httpOnly`, `SameSite=Lax`, `Path=/` session cookie
- [ ] 2.12 Implement `backend/src/auth/handlers.rs`: `GET /auth/login` handler — build EVE SSO redirect URL from `EsiMetadata.authorization_endpoint`, include CSRF state, redirect
- [ ] 2.13 Implement `GET /auth/callback` handler: validate state, exchange code for tokens via `EsiMetadata.token_endpoint`, parse access-token JWT for `eve_character_id` and `name`; fetch `corporation_id` / `alliance_id` from ESI public-info; in a single Postgres transaction — resolve or create the account (or use session's account in add-character mode), call `reactivate_if_soft_deleted`, call `upsert_character_from_login`; commit; insert/replace the in-memory session pointing to the resolved `account_id`; set cookie; redirect to `/`
- [ ] 2.14 Implement `GET /auth/logout` handler: remove session from store, clear cookie, redirect to `/`
- [ ] 2.15 Implement `GET /auth/characters/add` handler: require existing session (401 if absent), mark the session as `add_character_mode = true`, redirect to EVE SSO; the shared `/auth/callback` handler reads this flag to decide whether to reuse the session's account
- [ ] 2.16 Wire `AppState` in `backend/src/main.rs`: load config, `db::connect()` (which runs migrations), call `discover()` at startup (exit on failure for any of these), initialise `SessionStore`, build Axum router with all `/auth/*` routes
- [ ] 2.17 Verify `cargo build --release` produces zero warnings (use `SQLX_OFFLINE=true` with `cargo sqlx prepare` checked in, or rely on a running DB at build time — pick one and document)

## 2b. Backend: API key authentication

- [ ] 2b.1 Add `sha2` (or `ring`) and `base64` crates to `Cargo.toml`
- [ ] 2b.2 Implement `backend/src/api_key/mod.rs`:
  - `pub const PREFIX: &str = "erb_";`
  - `pub fn generate() -> String` — draw 32 bytes from a CSPRNG, base64url-encode unpadded (43 chars), return `format!("{PREFIX}{body}")`
  - `pub fn hash(key: &str) -> String` — SHA-256 hex digest of the full key
- [ ] 2b.3 Implement `backend/src/db/api_keys.rs`:
  - `create_account_key(account_id, name, expires_at) -> Result<(Uuid, String)>` — generates a key, inserts the row with `scope = 'account'`, returns `(id, plaintext_key)`. Plaintext exists only in the return value.
  - `lookup_by_key(plaintext: &str) -> Result<Option<ApiKeyRow>>` — `SELECT ... WHERE key_hash = $1 AND (expires_at IS NULL OR expires_at > now())`
  - `list_for_account(account_id) -> Result<Vec<ApiKeyMetadata>>` — no `key_hash` in the returned shape
  - `delete_for_account(id, account_id) -> Result<bool>` — `DELETE ... WHERE id = $1 AND account_id = $2`, returns whether a row was deleted
- [ ] 2b.4 Implement `backend/src/auth/middleware.rs`: an Axum extractor / middleware `AuthenticatedAccount(pub Uuid)`. On `/api/*`:
  1. If `Authorization: Bearer <value>` is present and starts with `erb_`: look up via `lookup_by_key`. On hit with `scope = 'account'` → set `account_id`; with `scope = 'server'` → reject 403; miss/expired → reject 401.
  2. Else fall back to session cookie. If neither → 401.
- [ ] 2b.5 Implement `backend/src/api/v1/keys.rs`:
  - `POST /api/v1/keys` — body `{ name, expires_at? }`; calls `create_account_key`; returns `201` with `id, key, name, expires_at, created_at`
  - `GET /api/v1/keys` — calls `list_for_account` for the caller's account
  - `DELETE /api/v1/keys/:id` — calls `delete_for_account`; `204` on success, `404` otherwise (row not found OR belongs to another account OR `scope = 'server'`)
- [ ] 2b.6 Mount the `/api/v1/keys` routes behind the `AuthenticatedAccount` middleware in `backend/src/main.rs`
- [ ] 2b.7 Verify with `curl`: create a key via session cookie; use the returned plaintext as `Authorization: Bearer …` to list keys; delete it; subsequent requests with that key return 401

## 2c. Backend: Account-management endpoints

- [ ] 2c.1 Implement `backend/src/esi/public_info.rs`: `fetch_corporation_name(corporation_id) -> Result<String>` and `fetch_alliance_name(alliance_id) -> Result<String>` against the ESI public-info endpoints discovered via the existing `EsiMetadata` flow (or the documented ESI base URL — pick one and note it). Both functions take `&reqwest::Client`; no caching in this change.
- [ ] 2c.2 Extend `backend/src/db/characters.rs`:
  - `count_for_account(account_id) -> Result<i64>` — for the `cannot_remove_last_character` check
  - `is_main(id) -> Result<Option<(Uuid, bool)>>` — returns `(account_id, is_main)` so the handler can verify ownership and main-status in one query
  - `set_main(tx, account_id, character_id)` already exists from 2.8 — ensure it is callable from a handler too (not only from the login flow)
  - When `upsert_character_from_login` inserts the **first** character for an account (count was zero before this call), set `is_main = TRUE` on the newly-inserted row. This satisfies the "first character auto-promoted to main" invariant in design.md §11. The promotion is part of the same transaction as the upsert.
- [ ] 2c.3 Extend `backend/src/db/accounts.rs`: `soft_delete` already exists from 2.7 — wire it into a new handler entry point. Add `list_sessions_for_account(account_id)` helper on `SessionStore` (or equivalent) so the soft-delete handler can drop every session belonging to the soft-deleted account.
- [ ] 2c.4 Implement `backend/src/api/v1/me.rs`:
  - `GET /api/v1/me` — load the caller's `account` row + all `eve_character` rows; for each character, resolve `corporation_name` and (when `alliance_id IS NOT NULL`) `alliance_name` via `esi::public_info`; build the response shape from `account-management/spec.md` (no token fields included). Wrap in the success envelope per `api-contract`.
- [ ] 2c.5 Implement `backend/src/api/v1/characters.rs`:
  - `POST /api/v1/characters/:id/set-main` — verify the character belongs to the caller (404 otherwise); call `set_main` in a transaction; reload and return the updated character (same shape as one element of `GET /api/v1/me`'s `characters` array, including resolved corp/alliance names and `portrait_url`).
  - `DELETE /api/v1/characters/:id` — verify ownership (404 otherwise); if `is_main = true` and the account has >1 character → 409 `cannot_remove_main`; if it is the only character → 409 `cannot_remove_last_character`; otherwise hard-delete the row and return 204.
- [ ] 2c.6 Implement `backend/src/api/v1/account.rs`:
  - `DELETE /api/v1/account` — in a single Postgres transaction call `accounts::soft_delete(caller.account_id)`. After commit, drop every in-memory session belonging to that account. Set a session-cookie-clearing `Set-Cookie` header on the response. Return 204.
  - Extend the auth middleware (or the per-route guard) so that an `Authorization: Bearer erb_…` whose `account.status = 'soft_deleted'` is rejected with HTTP 401 and `error.code = "account_soft_deleted"` (per account-management spec).
- [ ] 2c.7 Mount the new routes behind the `AuthenticatedAccount` middleware in `backend/src/main.rs` alongside `/api/v1/keys`.
- [ ] 2c.8 Verify with `curl`: `GET /api/v1/me` returns the expected shape after login; `POST /api/v1/characters/<id>/set-main` flips `is_main`; `DELETE /api/v1/characters/<main_id>` returns 409 while siblings exist; `DELETE /api/v1/characters/<only_id>` returns 409; `DELETE /api/v1/account` returns 204 and the cookie is cleared.

## 3. Backend: Dockerfile

- [ ] 3.1 Write `backend/Dockerfile`: multi-stage build — `rust:latest` builder stage compiling release binary, then `debian:bookworm-slim` runtime stage copying binary
- [ ] 3.2 Ensure `EXPOSE 3000` (or chosen internal port) and `CMD` are set correctly

## 4a. Wireframes (author and approve BEFORE frontend implementation)

These wireframes are the authoritative visual contract for section 4 (per design.md §10). Each file SHALL be a standalone, self-contained HTML page that can be opened in a browser with no build step. They SHALL inline the design tokens from design.md §10 (`--space-*`, slate scale, `--sky`, `--emerald`, `--red`) on `:root` and load JetBrains Mono from Google Fonts, so the rendered HTML is visually accurate to within ~5% of the final Svelte implementation. Use realistic placeholder data ("Wasp 223", "Artemisia de'Halicarnass", "Exit-Strategy", "Unchained Alliance") so the layout is reviewable.

The user reviews and approves these wireframes before any task in section 4 begins. Tweaks in this phase are cheap; tweaks after Svelte implementation are not.

- [ ] 4a.1 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/login.html` matching screenshot 01: centred card on `--space-950` background, cyan sun logo, `E-R BRIDGE` wordmark, "Wormhole Mapper" subtitle, divider, official EVE SSO PNG button (`https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png`), two-line disclaimer in `--slate-500`.
- [ ] 4a.2 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/home.html` matching screenshot 02: 48px GlobalNav at top with logo + brand + `maps` + `characters` links + pulsing emerald `connected` dot + user chip (24px portrait + name + chevron); centred-left body content reading `Welcome, Wasp 223` in `--sky`, then the main character name + `main` badge + corporation/alliance lines, then `Map view coming soon.`.
- [ ] 4a.3 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/characters.html` matching screenshot 04: same GlobalNav; page heading `CHARACTERS` in `--slate-500` uppercase + `+ add character` button top-right (`href="/auth/characters/add"`); two stacked character cards (one with `main` badge and no actions, one with right-aligned `set main` and `remove` text buttons); horizontal divider; `DANGER ZONE` section with `delete account` text button in `--red`.
- [ ] 4a.4 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/user-menu.html` matching screenshot 06: render `home.html`'s top-right chrome with the user-menu dropdown open beneath the user chip. Items: `preferences` (greyed-out, `aria-disabled="true"`), `settings` (greyed-out, `aria-disabled="true"`), divider, `log out` (`href="/auth/logout"`). Card surface is `--space-900` with `--space-700` border, anchored to the chip's right edge.
- [ ] 4a.5 The user opens each wireframe in a browser and signs off. If anything looks wrong (spacing, copy, palette, hover states, dropdown anchoring, badge style), the wireframe is updated before section 4 begins. The implementer SHALL NOT proceed to section 4 with un-approved wireframes.

## 4. Frontend: SvelteKit Project

All tasks in this section are blocked on §4a approval. Each task SHALL produce output that matches the corresponding wireframe; if a wireframe is silent on something (e.g. exact hover colour), it falls back to design.md §10/§11.

- [ ] 4.1 Scaffold SvelteKit app in `frontend/` using `npm create svelte@latest` with Svelte 5 and TypeScript
- [ ] 4.2 Install `@sveltejs/adapter-node`; update `svelte.config.js` to use it
- [ ] 4.3 Create `frontend/src/app.css`: import JetBrains Mono from Google Fonts; define all design-system CSS custom properties (`--space-950` through `--space-600`, full slate scale, `--sky`, `--emerald`, `--amber`, `--red`) on `:root` — names and values MUST match the wireframes' inlined tokens exactly so the Svelte build is pixel-equivalent; set `html, body` to `background: var(--space-950); color: var(--slate-100); font-family: "JetBrains Mono", ui-monospace, monospace; font-size: 13px`; import in `+layout.svelte`.
- [ ] 4.4 Create `frontend/src/lib/api.ts`: typed wrapper around the backend's `/api/v1/me`, `/api/v1/characters/:id/set-main`, `/api/v1/characters/:id`, and `/api/v1/account` endpoints. Each function returns the unwrapped `data` payload on success and throws a typed error for non-2xx responses (carrying `error.code` and `error.message` from the envelope). Types are derived from the `api-contract` machine-readable description (or hand-typed in this change with a TODO to switch to generated types).
- [ ] 4.5 Implement `frontend/src/routes/+layout.server.ts`: on every request, call backend `GET /api/v1/me` (forwarding the `cookie` header). On 401, redirect to `/login` unless the current route is `/login`. On 200, store the response in `event.locals.me`. If the request targets `/login` while `event.locals.me` is set, redirect to `/`.
- [ ] 4.6 Create `frontend/src/lib/components/GlobalNav.svelte` matching `wireframes/home.html`'s top bar exactly: 48px bar (`background: var(--space-900)`, `border-bottom: 1px solid var(--space-700)`); brand logo SVG in `--sky` + `E-R BRIDGE` wordmark linking to `/`; nav links `maps` (→ `/`) and `characters` (→ `/characters`), active link in `--sky` with a `--space-700` pill; right side: pulsing `--emerald` status dot + `connected` label (red + `disconnected` if `me` load failed); to its right, the `UserChip` component (4.7).
- [ ] 4.7 Create `frontend/src/lib/components/UserChip.svelte`: 24px circular portrait of the **main** character + main character name + chevron caret. Click toggles a `UserMenu` (4.8). Closes on outside-click and `Escape`.
- [ ] 4.8 Create `frontend/src/lib/components/UserMenu.svelte` matching `wireframes/user-menu.html`: dropdown card (`--space-900` background, `--space-700` border, anchored to the chip's right edge); items `preferences` (`aria-disabled="true"`, `tabindex="-1"`, no hover), `settings` (same), `--space-700` divider, `log out` (`<a href="/auth/logout">`).
- [ ] 4.9 Create `frontend/src/routes/+layout.svelte`: full-height shell (`display: flex; flex-direction: column; height: 100vh; overflow: hidden; background: var(--space-950)`); include `GlobalNav` at top **unless** the current route is `/login` (login has no nav per wireframe); yield `{@render children()}` for main content.
- [ ] 4.10 Implement `frontend/src/routes/login/+page.svelte` matching `wireframes/login.html`: full-viewport `--space-950` background; centred card (`background: var(--space-900)`, `border: 1px solid var(--space-700)`, `border-radius: 8px`); E-R Bridge logo SVG in `--sky`; wordmark + "Wormhole Mapper" subtitle; `<hr>` divider; `<a href="/auth/login"><img src="https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png" alt="LOG IN with EVE Online"></a>`; two-line disclaimer in `--slate-500`.
- [ ] 4.11 Implement `frontend/src/routes/+page.server.ts`: re-use `event.locals.me` from the layout (no separate fetch); pass `me` to the page as `data.me`.
- [ ] 4.12 Implement `frontend/src/routes/+page.svelte` (Svelte 5 syntax) matching `wireframes/home.html`: centred-left content area; `Welcome, <main.name>` heading in `--sky`; main character name + `main` badge + corporation row + alliance row (alliance row omitted when `alliance_id IS NULL`); `Map view coming soon.` placeholder. No sidebar. No canvas. The sidebar from the original design is deferred to the future map-rendering change.
- [ ] 4.13 Implement `frontend/src/routes/characters/+page.server.ts`: re-use `event.locals.me`; return `{ characters: locals.me.characters }`. Also expose form actions:
  - `setMain` — `POST /api/v1/characters/:id/set-main` then invalidate the layout load
  - `remove` — `DELETE /api/v1/characters/:id` then invalidate; surface `error.code` (`cannot_remove_main`, `cannot_remove_last_character`) as a user-visible message
  - `deleteAccount` — `DELETE /api/v1/account` then `redirect(303, '/login')`
- [ ] 4.14 Implement `frontend/src/routes/characters/+page.svelte` matching `wireframes/characters.html`: page heading `CHARACTERS` + `+ add character` link (`href="/auth/characters/add"`); stacked character cards rendered from `data.characters` (portrait from `portrait_url`, name, `main` badge for the main, corporation row, alliance row when present, `set main` + `remove` actions on non-main cards using the form actions from 4.13); divider; `DANGER ZONE` section with `delete account` form action button.
- [ ] 4.15 Verify `npm run build` produces a `build/` directory with a runnable Node.js server.
- [ ] 4.16 Open each implemented page side-by-side with its wireframe in a browser; the layouts SHALL be visually equivalent. Tune spacing, colours, and hover states until they match. Document any deliberate deviations as a comment in the relevant `+page.svelte`.

## 5. Frontend: Dockerfile

- [ ] 5.1 Write `frontend/Dockerfile`: multi-stage build — `node:lts` builder stage running `npm ci && npm run build`, then slim runtime stage running `node build`
- [ ] 5.2 Ensure `EXPOSE 3000` (or chosen port) and `CMD ["node", "build"]` are set

## 6. Traefik + Postgres Configuration

- [ ] 6.1 Write `traefik.yml` (static config): enable Docker provider, set entrypoint on port 80, disable dashboard in production
- [ ] 6.2 Write `docker-compose.yml` with four services:
  - `traefik`: mounts `/var/run/docker.sock` and `traefik.yml`; publishes port 80
  - `postgres`: `postgres:16` image; env vars `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` from `.env`; named volume `postgres-data:/var/lib/postgresql/data`; healthcheck using `pg_isready`
  - `backend`: built from `./backend`; env vars from `.env` including `DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}`; `depends_on: postgres: condition: service_healthy`; Traefik labels routing `/auth/` and `/api/` to it
  - `frontend`: built from `./frontend`; Traefik label routing all other requests to it
- [ ] 6.3 Declare the `postgres-data` named volume at the top level of `docker-compose.yml`
- [ ] 6.4 Confirm routing rules use `PathPrefix` matchers and that backend rule has higher priority than frontend catch-all

## 7. Integration Verification

- [ ] 7.1 Copy `.env.example` to `.env`, fill in real EVE SSO credentials and generated `ENCRYPTION_SECRET`
- [ ] 7.2 Run `docker compose up --build`; confirm all four containers reach running state and backend logs report migrations applied
- [ ] 7.3 Navigate to `APP_URL/`; confirm redirect to `/login`
- [ ] 7.4 Click the EVE SSO button; complete the SSO flow; confirm redirect to `/` shows the character name
- [ ] 7.5 Connect to Postgres (`docker compose exec postgres psql -U $POSTGRES_USER $POSTGRES_DB`); confirm one row in `account` (`status = 'active'`, `is_server_admin = false`) and one row in `eve_character` with non-NULL `encrypted_access_token`, `encrypted_refresh_token`, `esi_token_expires_at`, `esi_client_id`, `corporation_id`
- [ ] 7.6 Visit `/auth/characters/add`; complete SSO with a second EVE character; confirm a second row in `eve_character` linked to the same `account_id`
- [ ] 7.7 Run `docker compose down && docker compose up`; log in again with the same character; confirm the existing `eve_character` row is updated (same `id`, new `updated_at`, new ciphertext bytes due to fresh nonce) rather than duplicated
- [ ] 7.8 Soft-delete sanity: temporarily issue `UPDATE account SET status='soft_deleted', delete_requested_at = now()`; log in again; confirm `status` returns to `'active'` and `delete_requested_at` is NULL
- [ ] 7.9 Visit `APP_URL/auth/logout`; confirm redirect to `/login`
- [ ] 7.10 Verify session cookie is `httpOnly` and `SameSite=Lax` in browser devtools
- [ ] 7.11 Verify no token material (access or refresh) appears in any cookie or response header, and that plaintext tokens never appear in backend logs
- [ ] 7.12 API key end-to-end: while authenticated by session cookie, `curl -X POST $APP_URL/api/v1/keys -d '{"name":"smoke","expires_at":null}'`; capture returned plaintext `key`; confirm it matches `erb_[A-Za-z0-9_-]{43}`; confirm one row in `api_key` with `scope = 'account'`, matching `account_id`, and `key_hash` equal to `echo -n "$KEY" | sha256sum | cut -d' ' -f1`
- [ ] 7.13 API key authenticates: `curl -H "Authorization: Bearer $KEY" $APP_URL/api/v1/keys`; confirm the response lists the key and does NOT include a plaintext field
- [ ] 7.14 API key revocation: `curl -X DELETE -H "Authorization: Bearer $KEY" $APP_URL/api/v1/keys/$ID`; confirm 204; repeat the previous `curl` with the same key and confirm 401
- [ ] 7.15 Visit `/login` in a browser: layout matches `wireframes/login.html` (logo, wordmark, subtitle, EVE SSO button, disclaimer). The page has no GlobalNav.
- [ ] 7.16 After SSO, the browser lands on `/` and the layout matches `wireframes/home.html`: GlobalNav with brand + `maps` + `characters` + emerald `connected` dot + user chip (main character's portrait + name + chevron); body shows `Welcome, <main name>` plus the main character block plus `Map view coming soon.`
- [ ] 7.17 Click the user chip: dropdown appears matching `wireframes/user-menu.html` (`preferences` and `settings` greyed-out and non-interactive; divider; `log out`). Click outside or press `Escape`: dropdown closes. Click `log out`: redirected to `/login`.
- [ ] 7.18 Click `characters` in the nav: layout matches `wireframes/characters.html`. The main character's card has no actions; the non-main card has `set main` and `remove`. Click `+ add character`: redirected to EVE SSO; after completing with a third character, the new row appears in the list and the previous main is unchanged.
- [ ] 7.19 Click `set main` on a non-main character: the page reloads, the badge moves, and the user chip in the GlobalNav now shows the new main's portrait and name.
- [ ] 7.20 Attempt to `remove` the main character via direct API call (`curl -X DELETE`): confirm 409 `cannot_remove_main`. With only one character linked, attempt to remove it: confirm 409 `cannot_remove_last_character`.
- [ ] 7.21 Click `delete account` in the DANGER ZONE: response is 204, session cookie is cleared, browser redirects to `/login`. Confirm in Postgres that `account.status = 'soft_deleted'` and `delete_requested_at` is set; `eve_character` rows are untouched.
- [ ] 7.22 Log in again as the soft-deleted account's character: per existing 7.8, the account is reactivated. Visit `/`: the home page renders normally.
- [ ] 7.23 With a soft-deleted account (before re-login), attempt to use a previously-issued API key: confirm 401 with `error.code = "account_soft_deleted"`.
- [ ] 7.24 Open the rendered pages side-by-side with their wireframes. Any deviation that is not documented as deliberate is treated as a bug and fixed before this change is considered complete.
