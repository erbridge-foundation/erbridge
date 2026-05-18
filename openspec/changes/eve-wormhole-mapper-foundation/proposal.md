## Why

EVE Online wormhole space requires constant spatial awareness — pilots need to track which systems they've visited, how they connect, and which characters are in which system. No lightweight self-hosted tool exists to do this with proper EVE SSO integration. This change establishes the full project foundation so that wormhole chain mapping can be built on top.

## What Changes

- New monorepo with `/frontend` (SvelteKit + Svelte 5) and `/backend` (Rust/Axum) directories
- Docker Compose stack with four services: frontend, backend, Traefik v3 reverse proxy, Postgres 16
- Traefik routes `/auth/*` and `/api/*` to the backend; everything else to the frontend
- EVE ESI OAuth2 authentication in the backend: login, callback, logout, add-character endpoints
- Postgres persistence for durable identity: `account` (one row per signed-in user), `eve_character` (one or more rows per account, each holding encrypted ESI tokens), and `api_key` (programmatic access tokens); table names are singular by project convention
- SQL migrations managed via `sqlx::migrate!` and run automatically at backend startup
- API key authentication on `/api/*`: bearer tokens prefixed `erb_`, hashed with SHA-256 for storage; scopes `account` (acts as the owning account) and `server` (shape only — authorization semantics deferred)
- Management endpoints under `/api/v1/keys` (create / list / revoke) for the authenticated account to manage its own API keys
- Major-version prefix on all API routes (`/api/v1/...`) so future breaking changes can introduce `/api/v2/...` without disrupting existing clients
- Shared API contract for `/api/*`: `{ "data", "meta" }` success envelope, `{ "error": { "code", "message", "details" } }` error envelope, canonical error codes, RFC 3339 UTC timestamps
- OpenAPI 3.1 document derived from the running code via `utoipa`, served at `/api/openapi.json` with a Swagger UI at `/api/docs`; a backend test validates real handler responses against the document to prevent drift; frontend type generation from the document is deferred to a future change (the foundation ships hand-typed frontend types kept in sync with the OpenAPI doc)
- Session management: server-side session store (in-memory `HashMap` behind `Arc<RwLock<>>`) keyed by session ID and pointing to an `account_id`; session cookie (`httpOnly`, `SameSite=Lax`) — no token material exposed to the browser
- ESI refresh tokens encrypted at rest with AES-256-GCM before being stored in Postgres; HS256 JWT for the session cookie; both keys derived from `ENCRYPTION_SECRET`
- ESI SSO discovery document fetched at startup from the well-known endpoint and cached for process lifetime
- Frontend redirects unauthenticated requests to `/login`; authenticated routes (`/`, `/maps`, `/characters`) match the approved HTML wireframes (the `zz-ref/frontend/screenshots/` set was the source material for the wireframes but the wireframes are authoritative; notable deliberate divergences include: the nav exposes only `maps` and `characters` (no `acls`); the `/characters` page lays cards out in a 2-column grid with a search box and per-character token-status indicator; the home page is centred and omits the "Map view coming soon." line that now lives only on `/maps`)
- `/auth/login` and `/auth/characters/add` honour an optional `?return_to=<path>` hint (same-origin paths only; off-origin and scheme-relative values rejected); the callback redirects there on success — so e.g. clicking `+ add character` returns the user to `/characters` rather than `/`
- Visual design contract pinned by HTML wireframes (login / home / maps / characters / user-menu), authored and approved before any Svelte component is written; relocated to `frontend/wireframes/` before archival so they survive as a tracked, durable artefact
- Account-management API under `/api/v1/`: `GET /me` (returns the caller's account and character list with resolved corp/alliance names, portrait URLs, and a derived `token_status` enum per character — `"active"` when `esi_token_expires_at > now()`, `"expired"` otherwise — without exposing the raw timestamp), `POST /characters/:id/set-main`, `DELETE /characters/:id` (with `cannot_remove_main` / `cannot_remove_last_character` guards), `DELETE /account` (soft-delete with session cookie clearing)
- First-character auto-promotion: the first `eve_character` linked to an account is automatically flagged `is_main = TRUE` so the home page always has a main to render
- `.env.example` with all required environment variables documented, including `DATABASE_URL`

## Capabilities

### New Capabilities

- `eve-sso-auth`: EVE ESI OAuth2 authentication flow — login, callback, logout, add-character; server-side session and token management
- `project-infrastructure`: Monorepo layout, Docker Compose stack, Traefik v3 routing, environment variable configuration
- `data-persistence`: Postgres schema for accounts, EVE characters, and API keys; migration framework; encrypted-at-rest token storage
- `api-authentication`: Bearer-token API keys on `/api/*` with `Authorization` header authentication; management endpoints under `/api/keys`
- `api-contract`: Shared envelope (success + error), canonical error codes, timestamp format, and machine-readable route description for `/api/*` — consumed by both backend handlers and frontend client
- `account-management`: HTTP endpoints under `/api/v1/` for reading the authenticated account and its characters (`GET /me`), promoting a character to main, removing a character, and soft-deleting the account

### Modified Capabilities

## Impact

- New top-level directories: `/frontend`, `/backend`, `openspec/`
- New files: `docker-compose.yml`, `traefik.yml` (static config), `.env.example`, `Dockerfile`s for both services, `backend/migrations/*.sql`, HTML wireframes under `openspec/changes/eve-wormhole-mapper-foundation/wireframes/`
- Dependencies introduced: Axum, tokio, reqwest, sqlx (postgres + uuid + chrono features), aes-gcm, jsonwebtoken, thiserror, anyhow, utoipa, utoipa-swagger-ui (Rust); SvelteKit, @sveltejs/adapter-node, Svelte 5 (Node)
- New Postgres container in the Compose stack; persistent named volume for database data
- No existing code affected — this is a greenfield project
