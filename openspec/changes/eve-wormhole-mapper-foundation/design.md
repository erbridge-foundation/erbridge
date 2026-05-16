## Context

Greenfield EVE Online wormhole mapping tool. The backend must authenticate EVE pilots via EVE ESI OAuth2 (SSO), manage multi-character sessions, and expose a REST API for future wormhole chain features. The frontend is a thin SvelteKit SPA served by a Node adapter. Traefik routes requests so that a single public port reaches either service based on path prefix.

There are no existing services or databases to migrate. All runtime state for this change is in-memory; the structures are chosen for straightforward replacement with Postgres in a later change.

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
- Database persistence (deferred to a future change)
- Token refresh automation (tokens may expire; refresh is out of scope for this change)
- Frontend routing beyond a single `/` route

## Decisions

### 1. Traefik v3 as reverse proxy (not nginx)

Traefik's Docker provider auto-discovers services via container labels, eliminating manual upstream configuration. It handles TLS termination cleanly via ACME for a future production deployment. The routing rules (`PathPrefix`) map directly to the `/auth/*` and `/api/*` → backend, everything else → frontend requirement.

*Alternative considered: nginx with static upstream config.* Rejected because it requires manual updates when service ports change and doesn't integrate as cleanly with Docker Compose.

### 2. Session cookie with server-side token storage

The browser holds only an opaque session ID (AES-256-GCM encrypted, HS256 JWT signed). All OAuth2 tokens live in backend memory keyed by session ID. This eliminates token exposure to JavaScript and avoids PKCE complexity for a server-side flow.

*Alternative considered: storing tokens in a signed cookie.* Rejected — even encrypted, token material in the browser is higher risk and harder to revoke.

### 3. In-memory `HashMap<SessionId, Session>` behind `Arc<RwLock<>>`

Simple, zero-dependency state store. The `Session` struct is designed so that swapping to a Postgres-backed store later only requires changing the storage layer, not the auth logic (session ID remains the primary key, character list is a `Vec<Character>`).

*Alternative considered: `dashmap`.* Reasonable, but `Arc<RwLock<HashMap>>` is sufficient for expected traffic and makes the Postgres migration path obvious.

### 4. ESI SSO discovery at startup, cached in AppState

`reqwest::Client` fetches the well-known document once; `EsiMetadata` is stored directly in `AppState` (cloneable via `Arc`). This avoids repeated HTTP calls on every auth request and satisfies the "no hardcoded endpoint URLs" constraint.

### 5. SvelteKit with `@sveltejs/adapter-node`

The node adapter builds the frontend to a standalone Node.js server, which runs in its own Docker container. This is consistent with how the rest of the stack is containerized and avoids needing a separate static file server.

### 6. Cookie attributes: `httpOnly`, `SameSite=Lax`, `Path=/`

`httpOnly` prevents JavaScript from reading the session ID. `SameSite=Lax` allows the cookie to be sent on top-level navigation (needed for the OAuth2 redirect back to `/`) while blocking cross-site sub-resource requests. `Secure` is omitted for local development but SHOULD be set in production via configuration.

### 7. Derived keys from single `ENCRYPTION_SECRET`

A single `ENCRYPTION_SECRET` (32 hex bytes, 256 bits) is the root secret. The AES-256-GCM key and HS256 signing key are derived from it (or it is used directly as both, documented in code). This simplifies secret rotation — one variable to rotate.

### 8. License

The app will be licensed under AGPL-3.0.

## Risks / Trade-offs

- **In-memory state lost on restart** → Acceptable for this change; all active sessions are invalidated. Users re-login. Documented in `.env.example` comments.
- **No token refresh** → ESI tokens expire after ~20 minutes. Characters will need to re-authenticate. Acceptable at this stage; refresh is the next auth improvement.
- **Single Traefik instance, no HA** → This is a single-user/small-team tool; HA is not required.
- **`SameSite=Lax` without `Secure` in dev** → Cookie is transmitted over HTTP in local development. This is intentional and documented. Production deployment MUST add `Secure`.
- **`Arc<RwLock<>>` write contention under load** → Not a concern for expected usage (single pilot / small corp). Postgres migration resolves this if scale requires it.
