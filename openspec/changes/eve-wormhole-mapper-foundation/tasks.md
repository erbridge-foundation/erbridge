## 1. Repository Scaffold

- [ ] 1.1 Create root-level `frontend/` and `backend/` directories
- [ ] 1.2 Write `.env.example` with `APP_URL`, `ENCRYPTION_SECRET`, `ESI_CLIENT_ID`, `ESI_CLIENT_SECRET` — no secret defaults, comments for each
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
- [ ] 2.2 Add dependencies to `Cargo.toml`: `axum`, `tokio` (full), `reqwest` (json feature), `serde`/`serde_json`, `thiserror`, `anyhow`, `aes-gcm`, `jsonwebtoken`, `uuid`, `tower-http` (cors/trace), `dotenvy`
- [ ] 2.3 Implement `backend/src/esi/mod.rs` with `EsiMetadata` struct and `discover()` function verbatim as specified
- [ ] 2.4 Implement `backend/src/config.rs`: read `APP_URL`, `ENCRYPTION_SECRET`, `ESI_CLIENT_ID`, `ESI_CLIENT_SECRET` from env; fail fast with clear error if any are missing
- [ ] 2.5 Implement `backend/src/session.rs`: `Session` struct (session ID, `Vec<Character>`), `SessionStore` (`Arc<RwLock<HashMap<String, Session>>>`), add/remove/get helpers
- [ ] 2.6 Implement `backend/src/auth/crypto.rs`: AES-256-GCM encrypt/decrypt for session IDs; HS256 sign/verify for session cookie JWT; both keyed from `ENCRYPTION_SECRET`
- [ ] 2.7 Implement `backend/src/auth/cookie.rs`: helpers to create and clear the `httpOnly`, `SameSite=Lax`, `Path=/` session cookie
- [ ] 2.8 Implement `backend/src/auth/handlers.rs`: `GET /auth/login` handler — build EVE SSO redirect URL from `EsiMetadata.authorization_endpoint`, include CSRF state, redirect
- [ ] 2.9 Implement `GET /auth/callback` handler: validate state, exchange code for tokens via `EsiMetadata.token_endpoint`, store in `SessionStore`, set cookie, redirect to `/`
- [ ] 2.10 Implement `GET /auth/logout` handler: remove session from store, clear cookie, redirect to `/`
- [ ] 2.11 Implement `GET /auth/characters/add` handler: require existing session (401 if absent), redirect to EVE SSO; on callback append character to existing session
- [ ] 2.12 Wire `AppState` in `backend/src/main.rs`: call `discover()` at startup (exit on failure), initialise `SessionStore`, build Axum router with all `/auth/*` routes
- [ ] 2.13 Verify `cargo build --release` produces zero warnings

## 3. Backend: Dockerfile

- [ ] 3.1 Write `backend/Dockerfile`: multi-stage build — `rust:latest` builder stage compiling release binary, then `debian:bookworm-slim` runtime stage copying binary
- [ ] 3.2 Ensure `EXPOSE 3000` (or chosen internal port) and `CMD` are set correctly

## 4. Frontend: SvelteKit Project

- [ ] 4.1 Scaffold SvelteKit app in `frontend/` using `npm create svelte@latest` with Svelte 5 and TypeScript
- [ ] 4.2 Install `@sveltejs/adapter-node`; update `svelte.config.js` to use it
- [ ] 4.3 Implement `frontend/src/routes/+page.server.ts`: read session cookie from request headers; if present call backend session info endpoint (or decode opaque session indicator); pass `authenticated` flag and character list to page
- [ ] 4.4 Implement `frontend/src/routes/+page.svelte` (Svelte 5 syntax): if unauthenticated render login button linking to `/auth/login`; if authenticated render character name(s) and a logout link to `/auth/logout`
- [ ] 4.5 Verify `npm run build` produces a `build/` directory with a runnable Node.js server

## 5. Frontend: Dockerfile

- [ ] 5.1 Write `frontend/Dockerfile`: multi-stage build — `node:lts` builder stage running `npm ci && npm run build`, then slim runtime stage running `node build`
- [ ] 5.2 Ensure `EXPOSE 3000` (or chosen port) and `CMD ["node", "build"]` are set

## 6. Traefik Configuration

- [ ] 6.1 Write `traefik.yml` (static config): enable Docker provider, set entrypoint on port 80, disable dashboard in production
- [ ] 6.2 Write `docker-compose.yml` with three services:
  - `traefik`: mounts `/var/run/docker.sock` and `traefik.yml`; publishes port 80
  - `backend`: built from `./backend`; env vars from `.env`; Traefik labels routing `/auth/` and `/api/` to it
  - `frontend`: built from `./frontend`; Traefik label routing all other requests to it
- [ ] 6.3 Confirm routing rules use `PathPrefix` matchers and that backend rule has higher priority than frontend catch-all

## 7. Integration Verification

- [ ] 7.1 Copy `.env.example` to `.env`, fill in real EVE SSO credentials and generated `ENCRYPTION_SECRET`
- [ ] 7.2 Run `docker compose up --build`; confirm all three containers reach running state
- [ ] 7.3 Navigate to `APP_URL/`; confirm login button is shown
- [ ] 7.4 Click login; complete EVE SSO flow; confirm redirect to `/` shows character name
- [ ] 7.5 Visit `APP_URL/auth/logout`; confirm redirect to `/` shows login button again
- [ ] 7.6 Verify session cookie is `httpOnly` and `SameSite=Lax` in browser devtools
- [ ] 7.7 Verify no token material appears in any cookie or response header
