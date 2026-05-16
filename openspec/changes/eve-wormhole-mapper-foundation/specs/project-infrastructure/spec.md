## ADDED Requirements

### Requirement: Monorepo directory structure
The repository SHALL contain a `/frontend` directory with the SvelteKit application and a `/backend` directory with the Rust/Axum application. Each SHALL have its own `Dockerfile`. Common configuration (`.env.example`, `docker-compose.yml`) SHALL live at the repository root.

#### Scenario: Repository layout is correct
- **WHEN** the repository is cloned
- **THEN** `frontend/`, `backend/`, `docker-compose.yml`, and `.env.example` exist at the root

### Requirement: Docker Compose brings up the full stack
The repository SHALL include a `docker-compose.yml` that defines three services: `frontend`, `backend`, and `traefik`. Running `docker compose up --build` SHALL build both application images and start all three services without manual intervention.

#### Scenario: Single command starts the stack
- **WHEN** `docker compose up --build` is run from the repository root with a valid `.env` file
- **THEN** all three services start, health checks pass (or containers reach running state), and the application is reachable at `APP_URL`

### Requirement: Traefik v3 routes requests by path prefix
Traefik v3 SHALL be configured as the reverse proxy. Requests with path prefix `/auth/` or `/api/` SHALL be routed to the backend service. All other requests SHALL be routed to the frontend service. Traefik SHALL listen on port 80 (mapped from `APP_URL`'s port in Docker Compose).

#### Scenario: Auth requests reach the backend
- **WHEN** a request is made to `/auth/login`
- **THEN** Traefik forwards it to the backend container

#### Scenario: API requests reach the backend
- **WHEN** a request is made to `/api/anything`
- **THEN** Traefik forwards it to the backend container

#### Scenario: All other requests reach the frontend
- **WHEN** a request is made to `/` or any path not starting with `/auth/` or `/api/`
- **THEN** Traefik forwards it to the frontend container

### Requirement: Environment variables configure all secrets and URLs
All runtime configuration SHALL be supplied via environment variables. No secrets SHALL have default values. The repository SHALL include a `.env.example` file documenting all required variables with placeholder values and comments describing how to generate them.

Required variables:
- `APP_URL` — base URL used to construct the OAuth2 redirect URI
- `ENCRYPTION_SECRET` — 32-byte hex secret for AES-256-GCM and HS256 key derivation
- `ESI_CLIENT_ID` — EVE SSO application client ID
- `ESI_CLIENT_SECRET` — EVE SSO application client secret

#### Scenario: Missing ENCRYPTION_SECRET causes startup failure
- **WHEN** the backend starts without `ENCRYPTION_SECRET` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: .env.example documents all variables
- **WHEN** `.env.example` is read
- **THEN** all four required environment variables are present with placeholder values and explanatory comments

### Requirement: Frontend uses SvelteKit with node adapter and Svelte 5
The frontend SHALL be a SvelteKit application using `@sveltejs/adapter-node` and Svelte 5. The built output SHALL be a standalone Node.js server runnable inside a Docker container.

#### Scenario: Frontend builds and serves via Node adapter
- **WHEN** `npm run build` is run in the `frontend/` directory
- **THEN** a Node.js-runnable build artifact is produced in `build/`

### Requirement: Frontend root route is session-aware
The frontend SHALL have a single route at `/`. If the session cookie is present and valid (as confirmed by the backend or parsed client-side), the page SHALL display authenticated character information. If no valid session exists, the page SHALL display a login button linking to `/auth/login`.

#### Scenario: Unauthenticated user sees login button
- **WHEN** an unauthenticated browser requests `/`
- **THEN** the page renders a login button that navigates to `/auth/login`

#### Scenario: Authenticated user sees character info
- **WHEN** a browser with a valid session cookie requests `/`
- **THEN** the page renders the authenticated character's name (or list of characters for multi-character sessions)

### Requirement: Backend uses idiomatic Rust
The backend SHALL use latest stable Rust. Error handling SHALL use `thiserror` for custom error types. The code SHALL not use `unwrap()` or `expect()` in production paths. Unnecessary clones SHALL be avoided.

#### Scenario: Compilation succeeds with no warnings
- **WHEN** `cargo build --release` is run in the `backend/` directory
- **THEN** the build succeeds with zero compiler warnings
