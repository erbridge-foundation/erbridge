# E-R Bridge — Project Context

## What this is

E-R Bridge is a self-hosted wormhole mapping tool for EVE Online. Pilots authenticate with their EVE accounts via EVE SSO, link one or more characters to a session, and (in future changes) track wormhole chains, signatures, and pilot locations on a shared interactive map.

The name "E-R Bridge" is a play on Einstein-Rosen bridge — the theoretical construct EVE's wormholes are modelled after.

## Stack

- **Backend**: Rust (latest stable) on Axum, async via tokio
- **Frontend**: SvelteKit with Svelte 5 and the `@sveltejs/adapter-node`
- **Reverse proxy**: Traefik v3 (Docker provider)
- **Database**: Postgres 16, accessed via `sqlx` (compile-time-checked queries); migrations live in `backend/migrations/` and run at backend startup via `sqlx::migrate!`. **All table names are singular** (`account`, `eve_character`, …) — never pluralised.
- **Containerization**: Docker Compose for the full stack — `docker compose up --build` brings up frontend, backend, Traefik, and Postgres
- **External APIs**: EVE ESI (`https://esi.evetech.net`) for game data; EVE SSO (`https://login.eveonline.com`) for OAuth2; anoikis.info for static wormhole data (`https://anoikis.info/data/wh-statics.json`), cached and refreshed daily
- **Persistence model**:
  - **Postgres** holds durable identity, tokens, and API keys: `account`, `eve_character`, and `api_key`. Both ESI access and refresh tokens are stored encrypted at rest with AES-256-GCM in `eve_character.encrypted_access_token` / `encrypted_refresh_token`. Postgres is the single source of truth for tokens. API keys are stored as `SHA-256` hex hashes.
  - **In-memory** (`Arc<RwLock<HashMap>>`) holds only ephemeral session routing: session ID → `account_id` plus CSRF state for in-flight OAuth2 redirects. No token material lives here. Sessions are intentionally lost on restart; users re-login

## Repository layout

```
/backend         Rust/Axum service — auth + future REST API
  /migrations    SQL migrations applied at startup via sqlx::migrate!
/frontend        SvelteKit app — UI
/openspec        Spec-driven development artefacts (this directory)
/zz-ref          Reference material (wireframes, screenshots, design notes) — not shipped
docker-compose.yml
traefik.yml
.env.example
```

## Routing model

Traefik v3 sits in front of both services. Path-prefix routing:

- `/auth/*` → backend (OAuth2 endpoints; session-cookie auth)
- `/api/*` → backend (REST API; session-cookie OR `Authorization: Bearer erb_…` API-key auth)
- everything else → frontend (SvelteKit Node server)

## Authentication model

- **Browser-facing**: opaque, encrypted, signed session ID cookie (`httpOnly`, `SameSite=Lax`). The cookie carries only a session ID — no token material
- **Programmatic**: bearer-token API keys with the `erb_` prefix on `/api/*`. Stored as SHA-256 hashes in `api_key`. Scopes: `account` (acts as the owning account) and `server` (reserved; no current authority)
- All EVE OAuth2 tokens (access + refresh) live server-side encrypted at rest in `eve_character`; Postgres is the single source of truth for tokens
- Multi-character: one account can link many EVE characters via `/auth/characters/add`. In-memory sessions just hold `session_id → account_id`
- ESI SSO discovery document is fetched once at startup from the well-known endpoint and cached in `AppState`. Endpoint URLs (`authorization_endpoint`, `token_endpoint`, `jwks_uri`) MUST be derived from the discovery document — never hardcoded
- Cryptography: AES-256-GCM for tokens at rest and session cookie payload; HS256 for session cookie JWT signing; all keys derived from `ENCRYPTION_SECRET`

## Configuration

All runtime config is via environment variables (see `.env.example`). No secret defaults. Required:

- `APP_URL` — base URL, used to construct the OAuth2 redirect URI `{APP_URL}/auth/callback`
- `ENCRYPTION_SECRET` — 32 hex bytes; the single root secret for refresh-token-at-rest encryption, session cookie encryption, and session cookie JWT signing
- `ESI_CLIENT_ID`, `ESI_CLIENT_SECRET` — registered at https://developers.eveonline.com
- `DATABASE_URL` — Postgres DSN consumed by the backend
- `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` — consumed by the Postgres container and referenced from `DATABASE_URL`

## Database conventions

- **Table names are singular** — `account`, `eve_character`, `map`, `map_signature`, never the plural forms
- Primary keys are `UUID` v7 defaulting to `gen_random_uuid()` (requires `pgcrypto`), except where an external system's ID is the natural key
- `created_at` and `updated_at` `TIMESTAMPTZ NOT NULL DEFAULT now()` on every table
- Foreign keys use `ON DELETE CASCADE` where the child row has no meaning without its parent
- Indexes are named `<table>_<column>_idx` (e.g. `eve_character_account_id_idx`)
- Migrations are timestamp-prefixed plain SQL in `backend/migrations/`, descriptive snake_case names (`20260516120000_create_account_and_eve_character.sql`)

## Stack-specific coding conventions

Stack-specific rules live in skills, not here:

- **Backend (Rust/Axum)** — load the `rust-rest-api` skill (`.claude/skills/rust-rest-api/SKILL.md`) before writing or reviewing any backend Rust. Authoritative source for layered architecture (handler → service → db), DTO rules, response envelope, error handling, and test-coverage requirements.
- **Frontend (SvelteKit + Svelte 5)** — load the `sveltekit-node` skill (`.claude/skills/sveltekit-node/SKILL.md`) before writing or reviewing any frontend code. Authoritative source for SvelteKit patterns, Svelte 5 rune usage, the native-CSS / design-token system, load functions, form actions, server endpoints, and Svelte Flow conventions.

Conventional commits across the whole repo.

## Design language

The visual design is defined by the wireframe at `zz-ref/frontend/wireframes/map_canvas.html` and the screenshots in `zz-ref/frontend/screenshots/`. Key tokens:

- Backgrounds: deep navy `--space-950` → `--space-600` scale
- Text: slate scale `--slate-100` → `--slate-600`
- Accents: `--sky` (brand), `--emerald` (online/positive), `--amber` (warning), `--red` (destructive), `--violet` (named-root pill)

Full token table and component specs live in `specs/project-infrastructure/spec.md`.

## What's out of scope (for now)

These are deliberately deferred to future change proposals:

- Wormhole chain visualization / map canvas
- Signature scanning
- Domain tables beyond `account`, `eve_character` and `api_key`
- Persistent sessions across restarts
- Token refresh automation (refresh tokens are stored encrypted but not yet used)
- `ENCRYPTION_SECRET` rotation tooling (manual procedure documented; no automation)
- Production deployment (TLS, `Secure` cookie attribute, observability)

## License

AGPL-3.0.
