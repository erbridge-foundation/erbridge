## Why

EVE Online wormhole space requires constant spatial awareness — pilots need to track which systems they've visited, how they connect, and which characters are in which system. No lightweight self-hosted tool exists to do this with proper EVE SSO integration. This change establishes the full project foundation so that wormhole chain mapping can be built on top.

## What Changes

- New monorepo with `/frontend` (SvelteKit + Svelte 5) and `/backend` (Rust/Axum) directories
- Docker Compose stack with three services: frontend, backend, Traefik v3 reverse proxy
- Traefik routes `/auth/*` and `/api/*` to the backend; everything else to the frontend
- EVE ESI OAuth2 authentication in the backend: login, callback, logout, add-character endpoints
- Session management: server-side token storage (in-memory `HashMap` behind `Arc<RwLock<>>`), session cookie (`httpOnly`, `SameSite=Lax`) — no token material exposed to the browser
- Sessions encrypted with AES-256-GCM; JWT signed with HS256; both keys derived from `ENCRYPTION_SECRET`
- ESI SSO discovery document fetched at startup from the well-known endpoint and cached for process lifetime
- Frontend single route at `/`: shows login button or authenticated character info based on session cookie
- `.env.example` with all required environment variables documented

## Capabilities

### New Capabilities

- `eve-sso-auth`: EVE ESI OAuth2 authentication flow — login, callback, logout, add-character; server-side session and token management
- `project-infrastructure`: Monorepo layout, Docker Compose stack, Traefik v3 routing, environment variable configuration

### Modified Capabilities

## Impact

- New top-level directories: `/frontend`, `/backend`, `openspec/`
- New files: `docker-compose.yml`, `traefik.yml` (static config), `.env.example`, `Dockerfile`s for both services
- Dependencies introduced: Axum, tokio, reqwest, tower-sessions, aes-gcm, jsonwebtoken, thiserror, anyhow (Rust); SvelteKit, @sveltejs/adapter-node, Svelte 5 (Node)
- No existing code affected — this is a greenfield project
