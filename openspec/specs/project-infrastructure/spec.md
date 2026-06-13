## Purpose

Monorepo layout (`/frontend`, `/backend`), Docker Compose stack (frontend, backend, Traefik v3, Postgres 16), Traefik path-prefix routing, environment-variable configuration, frontend SvelteKit + Svelte 5 baseline, the `/login` page, the project-wide design system, and the backend's Rust idiom rules.
## Requirements
### Requirement: Monorepo directory structure
The repository SHALL contain a `/frontend` directory with the SvelteKit application and a `/backend` directory with the Rust/Axum application. Each SHALL have its own `Dockerfile`. Common configuration (`.env.example`, `docker-compose.yml`) SHALL live at the repository root.

#### Scenario: Repository layout is correct
- **WHEN** the repository is cloned
- **THEN** `frontend/`, `backend/`, `docker-compose.yml`, and `.env.example` exist at the root

### Requirement: Docker Compose brings up the full stack
The repository SHALL include a `docker-compose.yml` that defines four services: `frontend`, `backend`, `traefik`, and `postgres`. Running `docker compose up --build` SHALL build both application images and start all four services without manual intervention. The `backend` service SHALL declare `depends_on` for `postgres` with `condition: service_healthy`.

#### Scenario: Single command starts the stack
- **WHEN** `docker compose up --build` is run from the repository root with a valid `.env` file
- **THEN** all four services start, the Postgres healthcheck passes before the backend starts, and the application is reachable at `APP_URL`

### Requirement: Traefik v3 routes requests by path prefix
Traefik v3 SHALL be configured as the reverse proxy. Requests with path prefix `/auth/` or `/api/` SHALL be routed to the backend service. All other requests SHALL be routed to the frontend service. Traefik SHALL listen on port 80 (mapped from `APP_URL`'s port in Docker Compose).

#### Scenario: Auth requests reach the backend
- **WHEN** a request is made to `/auth/anything`
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
- `ENCRYPTION_SECRET` — 32-byte hex secret for AES-256-GCM (refresh tokens + session cookie payload) and HS256 (session cookie JWT) key derivation
- `ESI_CLIENT_ID` — EVE SSO application client ID
- `ESI_CLIENT_SECRET` — EVE SSO application client secret
- `DATABASE_URL` — Postgres connection string (`postgres://user:pass@host:port/dbname`) used by the backend
- `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` — consumed by the Postgres container and referenced from `DATABASE_URL`

#### Scenario: Missing APP_URL causes startup failure
- **WHEN** the backend starts without `APP_URL` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: Missing ENCRYPTION_SECRET causes startup failure
- **WHEN** the backend starts without `ENCRYPTION_SECRET` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: Missing ENCRYPTION_SECRET causes startup failure
- **WHEN** the backend starts without `ESI_CLIENT_ID` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: Missing ENCRYPTION_SECRET causes startup failure
- **WHEN** the backend starts without `ESI_CLIENT_SECRET` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: .env.example documents all variables
- **WHEN** `.env.example` is read
- **THEN** all four required environment variables are present with placeholder values and explanatory comments

### Requirement: Frontend uses SvelteKit with node adapter and Svelte 5
The frontend SHALL be a SvelteKit application using `@sveltejs/adapter-node` and Svelte 5. The built output SHALL be a standalone Node.js server runnable inside a Docker container.

#### Scenario: Frontend builds and serves via Node adapter
- **WHEN** `npm run build` is run in the `frontend/` directory
- **THEN** a Node.js-runnable build artifact is produced in `build/`

### Requirement: Unauthenticated users are redirected to /login
The frontend SHALL redirect any unauthenticated request for `/` (or any protected route) to `/login` via a SvelteKit server-side redirect. The `/login` route SHALL NOT be accessible to authenticated users — they SHALL be redirected to `/`.

#### Scenario: Unauthenticated user visiting / is redirected
- **WHEN** an unauthenticated browser requests `/`
- **THEN** the server responds with a redirect to `/login`

#### Scenario: Authenticated user visiting /login is redirected
- **WHEN** a browser with a valid session cookie requests `/login`
- **THEN** the server responds with a redirect to `/`

### Requirement: /login page matches the reference screenshot
The `/login` route SHALL render a vertically and horizontally centred card on a full-viewport `--space-950` background (no nav bar). The card SHALL match the layout in `zz-ref/frontend/screenshots/01_login_page.png`:

- Card: `background: var(--space-900)`, `border: 1px solid var(--space-700)`, `border-radius: 8px`, content centred, generous vertical padding
- Top of card: the E-R Bridge logo SVG in `--sky`, centred
- Below logo: wordmark `E-R BRIDGE` in `0.875rem` / `600` weight with `letter-spacing: 0.2em`, `color: var(--slate-100)`
- Below wordmark: subtitle `Wormhole Mapper` in 11px, `color: var(--slate-400)`
- Horizontal rule separating the branding from the login action: `border-color: var(--space-700)`
- Login button: an `<a href="/auth/login">` wrapping an `<img>` loaded from `https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png` (the official EVE SSO button image); no custom button styling applied — the image is the button
- Below the button: two lines of small muted text centred, `color: var(--slate-500)`: "Authentication is handled by EVE Online." and "No password is stored by this service."

#### Scenario: /login page renders the card layout
- **WHEN** an unauthenticated browser requests `/login`
- **THEN** the page shows a centred card on a `--space-950` background with the E-R Bridge logo, wordmark, subtitle, EVE SSO button image, and disclaimer text

#### Scenario: EVE SSO button image is loaded from CCP CDN
- **WHEN** the `/login` page is rendered
- **THEN** the login button is an `<img>` with `src="https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png"` wrapped in an anchor linking to `/auth/login`

#### Scenario: Authenticated user sees character info on /
- **WHEN** a browser with a valid session cookie requests `/`
- **THEN** the page renders the authenticated character's name (or list of characters for multi-character sessions)

### Requirement: Frontend applies the E-R Bridge design system
The frontend SHALL implement the visual design language established in the wireframe. All pages and components SHALL use this system consistently.

**Typography**
The default typeface is JetBrains Mono (Google Fonts), applied globally via a `--font-ui` custom property (defaulting to `"JetBrains Mono", ui-monospace, monospace`) that `<body>` and everything inheriting from it use. The indirection exists so the `dyslexia_font` accessibility preference can swap the whole UI to an alternative typeface (Atkinson Hyperlegible) by overriding `--font-ui` on `<html>` (see the `accessibility-preferences` capability). The `<html>` element's `font-size` defaults to `100%` (picking up the browser/OS default, typically 16px) but is **user-controllable** via the `text_size` accessibility preference, which overrides `html { font-size }`. The `<body>` SHALL be set to `font-size: 0.875rem` (≈14px at the default). **All typography rules across the design system SHALL be expressed in `rem`, not `px`**, so the UI scales both when a visitor changes their browser font-size or zooms AND when they change the `text_size` preference. Spacing (padding, margin, gap, border-radius, avatar/icon dimensions, border widths) is exempt and SHALL remain in `px`, since those are visual-layout values that must not grow with text-size preferences.

**Motion**
All animations and transitions SHALL be gated on the reduce-motion preference (which defaults to the OS `prefers-reduced-motion` setting), so motion in the design system — including the pulsing status dot — does not bypass the accessibility preference (see the `accessibility-preferences` capability).

**Colour tokens** — defined as CSS custom properties on `:root`:

| Token | Value | Role |
|---|---|---|
| `--space-950` | `#05080f` | Page / canvas background |
| `--space-900` | `#080d1a` | Surface: nav, sidebar, panels |
| `--space-800` | `#0d1526` | Raised surface: inputs, nodes |
| `--space-700` | `#152238` | Borders, dividers |
| `--space-600` | `#1e3352` | Subtle borders, input outlines |
| `--slate-100` | `#f1f5f9` | Primary text |
| `--slate-200` | `#e2e8f0` | Hover text |
| `--slate-300` | `#cbd5e1` | Secondary text |
| `--slate-400` | `#94a3b8` | Muted text, nav links |
| `--slate-500` | `#64748b` | Placeholder, icon resting |
| `--slate-600` | `#475569` | Disabled / count labels |
| `--emerald` | `#10b981` | Online status, positive actions |
| `--amber` | `#f59e0b` | Warning, history mode |
| `--red` | `#ef4444` | Destructive, critical mass |
| `--sky` | `#38bdf8` | Brand accent (logo, active tab indicator) |
| `--violet` | `#a78bfa` | Named-root pill, code |

**Global nav bar** (`height: 48px`, `background: var(--space-900)`, `border-bottom: 1px solid var(--space-700)`):
- Left: brand logo SVG in `--sky` + wordmark `E-R BRIDGE` in 12px/600 weight with `letter-spacing: 0.2em`, separated from nav links by a `1px solid var(--space-700)` rule
- Nav links: 11px, `color: var(--slate-400)`; on hover/active: `color: var(--slate-200)`, `background: var(--space-700)`, `border-radius: 4px`; height 28px, padding `0 12px`
- Right side: pulsing emerald status dot + "connected" label; find-system input (`width: 180px`, `background: var(--space-800)`, `border: 1px solid var(--space-600)`); icon-only logout button

**Left sidebar** (`width: 288px`, collapsible to 40px icon rail):
- `background: var(--space-900)`, `border-right: 1px solid var(--space-700)`
- Collapsible via a 24px circular toggle button that overflows the sidebar's right edge (`right: -12px`); icon rotates 180° when collapsed
- Collapsed state: hides all text, section bodies, and counts — shows only section icon in a centred rail
- Section headers: 10px uppercase `letter-spacing: 0.08em`, `color: var(--slate-400)`; include a chevron (rotates 90° when open), a title, an optional count in `--slate-600`, and an optional action icon button (`color: var(--slate-500)`, hover `color: var(--emerald)`)
- Sections separated by `border-bottom: 1px solid var(--space-700)`
- Sidebar scrolls vertically with `scrollbar-width: thin; scrollbar-color: var(--space-600) transparent`

**Login / unauthenticated page** — applies the same shell: full-height dark background (`--space-950`). if a user is not authenticated, they SHALL redirect to this /login page. the main area SHALL centre the login call-to-action.

#### Scenario: Global nav renders correctly on all pages
- **WHEN** any page is loaded
- **THEN** the nav bar is 48px tall with `--space-900` background, brand logo in `--sky`, and nav links styled per the design system

#### Scenario: Login page uses the design system
- **WHEN** an unauthenticated user loads `/login`
- **THEN** the page background is `--space-950`, the nav bar is NOT present

#### Scenario: Sidebar collapses to icon rail
- **WHEN** the sidebar toggle is clicked on the authenticated map view
- **THEN** the sidebar width transitions to 40px, text is hidden, and only section icons remain visible

#### Scenario: Text size preference scales typography
- **WHEN** a user sets the `text_size` accessibility preference away from its default
- **THEN** `html { font-size }` changes and all `rem`-based typography scales proportionally, while `px` spacing values are unaffected

### Requirement: Backend uses idiomatic Rust
The backend SHALL use latest stable Rust. Error handling SHALL use `thiserror` for custom error types. The code SHALL not use `unwrap()` or `expect()` in production paths. Unnecessary clones SHALL be avoided.

#### Scenario: Compilation succeeds with no warnings
- **WHEN** `cargo build --release` is run in the `backend/` directory
- **THEN** the build succeeds with zero compiler warnings

### Requirement: Backend service lifecycle and request limits

The backend SHALL shut down gracefully on SIGTERM and SIGINT: stop accepting new connections, allow in-flight requests to complete, then exit — a routine deploy MUST NOT sever requests mid-flight. The backend SHALL bound request duration with a timeout layer (30 seconds) so a stalled upstream cannot hold connections open indefinitely. The listen address SHALL be configurable via a `BIND_ADDR` environment variable, defaulting to `0.0.0.0:3000` so existing deployments need no configuration change.

#### Scenario: SIGTERM drains in-flight requests

- **WHEN** the process receives SIGTERM while a request is in flight
- **THEN** that request completes and receives its response, no new connections are accepted, and the process then exits

#### Scenario: A hung request is terminated by the timeout

- **WHEN** a request's handler does not produce a response within the timeout
- **THEN** the connection receives an error response rather than hanging indefinitely

#### Scenario: Default bind address is unchanged

- **WHEN** the backend starts with no `BIND_ADDR` set
- **THEN** it listens on `0.0.0.0:3000` exactly as before

