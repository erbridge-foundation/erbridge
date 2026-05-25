## Context

The foundation change (`eve-wormhole-mapper-foundation`) ships the app's auth, identity, and characters surfaces and bakes in the `api-contract` rule that every `/api/*` response uses a `{ data, meta? }` success envelope (with a single carve-out for `/auth/*`). It does not include an about page or any health endpoint.

Three things now want an answer in the same change because they are tightly coupled:

1. **Where does the API version come from?** Hard-coding it on the frontend means the displayed version can lag the running binary. A small `/health` endpoint that returns `CARGO_PKG_VERSION` is the obvious source of truth.
2. **Where do the legal disclaimer and acknowledgements live?** EVE third-party developers must display CCP's standard disclaimer somewhere the user can find. Burying it in CLAUDE.md is wrong; surfacing it in the user-menu under `about` is the conventional place.
3. **How do orchestration tools (Traefik, future k8s probes, uptime monitors) probe liveness?** The same endpoint that drives the about page can serve their needs if it returns a shape that's friendly to shallow parsing.

Goals for this change are therefore both UX (a polished about page) and infra (a real health endpoint). Both are small, but the interaction with the api-contract envelope rule needs a deliberate decision, captured below.

**As-built auth model (correcting an earlier assumption in this change).** Earlier drafts of this change described mounting `/api/health` on a "public router branch" outside an "`AuthenticatedAccount` middleware tree." The foundation did **not** build auth that way. The router is assembled in `backend/src/lib.rs::build_router` (not `main.rs`), and the **only** global middleware layers are `refresh_session_cookie` (cookie-refresh, cross-cutting) and `TraceLayer`. There is no public/authenticated router split. Auth is enforced **per-handler** by the `AuthenticatedAccount` extractor (`backend/src/handlers/middleware.rs`): a handler that names `AuthenticatedAccount(account_id): AuthenticatedAccount` in its signature is authenticated and receives the typed account id; a handler that omits it is public. The auth *logic* is centralised in one place (the extractor's `from_request_parts`); what is per-handler is the one-line *declaration* of the dependency. This makes `/api/health` trivially public — it simply does not name the extractor — and the tasks/specs below are written to that model, not the imagined middleware tree. The one weakness of an opt-in auth model (forgetting the extractor fails *open*) is addressed by Decision §10.

## Goals / Non-Goals

**Goals:**

- A single new `/about` page in the SvelteKit app, reachable from the user-menu dropdown, that satisfies the EVE third-party legal-disclaimer expectation.
- A real `GET /api/health` endpoint that returns version, commit, and per-component status (initially just `db`); usable by both the about page and external probes.
- The about page renders correctly even when `/api/health` fails (degraded state — show UI version, show "API: unreachable", still render legal + acknowledgements).
- The api-contract spec is amended (not violated) to permit `/api/health` to return flat JSON; the carve-out is symmetric with the existing `/auth/*` carve-out.

**Non-Goals:**

- A metrics endpoint (`/metrics`, Prometheus, etc.) — `/api/health` returns a coarse status, not time-series data.
- Detailed per-component health (e.g. ESI reachability, Redis latency, queue depth). The schema supports more components arriving later via the `components` array, but this change ships only `db`.
- A versioned health endpoint (`/api/v1/health`). Health is a build-level concern, not a versioned API surface; it lives at `/api/health` and stays there.
- Authenticated-only health. Health is intentionally public so external probes work.
- A "what's new" / changelog surface on the about page. Acknowledgements + version + legal only; a release-notes section can land later when there are actually releases.
- Per-character ESI scope status, ESI quota indicators, or any account-specific data on the about page. About is global / static — same content for every visitor.

## Decisions

### 1. `/api/health` is exempt from the success envelope

The api-contract spec freezes the `{ data, meta? }` envelope for `/api/*`. We carve `/api/health` out with a single MODIFIED requirement in this change, parallel in structure to the existing `/auth/*` carve-out. The endpoint returns:

```json
{
  "status": "ok" | "degraded",
  "version": "0.1.0",
  "commit": "f7fad77",
  "components": [
    { "name": "db", "status": "ok" }
  ]
}
```

**Why a carve-out, not enveloping the response?** Industry convention for `/health` is a flat shape; orchestration tools (k8s liveness/readiness, Traefik healthcheck, uptime monitors) typically shallow-parse `status`. Forcing them to read `data.status` is a friction tax with no upside. The api-contract spec already accepts that uniformity has limits (`/auth/*` is exempt because HTML redirects don't fit JSON envelopes); `/api/health` joins that category.

**Why not mount at `/health` (outside `/api/*`)?** Considered. Two reasons to keep it under `/api/`:

1. The OpenAPI doc and Swagger UI already live at `/api/openapi.json` and `/api/docs`. Putting health at `/api/health` keeps all the machine-readable / observability endpoints in one tree.
2. The carve-out is now a documented exception with a stable contract, not a special case "outside the system." Anyone reading `api-contract/spec.md` will see exactly one paragraph explaining why `/api/health` is flat.

*Alternative considered:* leave `/api/health` enveloped, document that callers must read `data.status`. Rejected — the friction-tax argument above.

*Alternative considered:* move health to `/health`. Rejected — splits the observability surface for no concrete gain.

### 2. Overall status is derived, not a separate column to assert against

`status` at the top level SHALL be `"ok"` if and only if every component's `status` is `"ok"`; otherwise `"degraded"`. The aggregation lives in the handler, not the DB. This means callers and tests cannot disagree about what "the overall status" means — there is exactly one rule and it's executed in one place.

A small consequence: a brand-new component being added in a future change (e.g. `esi`) immediately participates in the overall status calculation. That is the intended behaviour; the handler picks up any new component without a special case.

### 3. `commit` is captured at build time via `build.rs`

A small `backend/build.rs` emits `cargo:rustc-env=GIT_COMMIT_SHA=<sha>`; the handler reads it via `env!("GIT_COMMIT_SHA")`. Resolution order in the script: (1) an explicit `GIT_COMMIT_SHA` build-time env var if non-empty, (2) `git rev-parse --short HEAD` against a local `.git/`, (3) the literal `"unknown"`. The script SHALL never panic.

Build environments and the SHA they produce:

- **Local / repo build with `.git/`**: the real short SHA (path 2).
- **Source distribution without `.git/`** (e.g. an unpacked tarball): falls through to `"unknown"` (path 3). `/api/health` simply returns `"commit": "unknown"`.
- **Docker build**: the backend Dockerfile is a multi-stage build that copies *named files* into the builder stage — it does **not** copy `.git/`. Two requirements follow, and both were corrected during implementation:
  1. `build.rs` MUST be copied into the builder stage. Cargo only runs a build script if `build.rs` is present in the crate root; the foundation Dockerfile copied `Cargo.toml`/`src`/`migrations`/`.sqlx` but **not** `build.rs`, so the script never ran and `env!("GIT_COMMIT_SHA")` failed to compile. The Dockerfile's `COPY` line now includes `build.rs`.
  2. With no `.git/` in the context, the Docker build legitimately resolves to `"unknown"` (path 3) — this is acceptable. If a deployment wants a real SHA in the image, pass it via the `GIT_COMMIT_SHA` build arg (path 1) without any code change; the build script already honours it.

*Alternative considered:* read the commit at runtime via `git2` or by shelling out. Rejected — the handler must be free of process state, and a binary that ships separately from its build tree can't read git history anyway. Build-time injection is the only correct answer.

*Alternative considered:* hard-code the commit in `Cargo.toml` and bump it by hand. Rejected — humans will forget. The build script makes it automatic.

### 4. `db` component health is a `SELECT 1` ping in the handler

The handler performs `sqlx::query!("SELECT 1").execute(&pool).await` (or equivalent) and maps `Ok(_)` to `"ok"`, `Err(_)` to `"degraded"`. No metrics, no latency tracking, no retry. The handler runs on every health probe, so the call must be cheap and bounded.

**Why not skip the DB ping?** Because liveness without DB reachability is meaningless for this app — if Postgres is unreachable, no `/auth/*` or `/api/v1/*` request can succeed. An honest health endpoint says so.

**Why not cache the result?** Because health probes happen on a cadence that matches the cache TTL anyway. Caching would just delay the degraded signal. If `SELECT 1` becomes a measurable load problem (it won't, at this scale), we can add a 1-second TTL later.

### 5. About page is server-rendered via `+page.server.ts`

The page calls `/api/health` from `+page.server.ts` (server-side fetch), so the response shape is parsed once on the server and the client just renders. This:

- Avoids a flash of "loading…" on the version + commit fields.
- Means the page degrades gracefully on health-fetch failure — the server load returns `{ health: null, healthError: { message } }` and the Svelte component renders "API: unreachable" inline. This mirrors the `+layout.server.ts` pattern from the foundation (§4.5).
- Does NOT *consume* `+layout.server.ts`'s `locals.me` — about must render for unauthenticated visitors too, so its content is independent of the session. (The layout load still runs and still calls `getMe`; for an anonymous visitor it resolves to `me: null`. The about page simply does not read that value.) The auth gate is the layout's redirect on a 401 from `getMe`, which the change relaxes for `/about` below.

A wrinkle: foundation's `+layout.server.ts` redirects to `/login` whenever `getMe` returns 401, *except* on `/login` itself — and it expresses this with a single boolean `isLoginRoute = url.pathname === '/login'`, **not** an allowlist array (an earlier draft of this change assumed an array). To make `/about` reachable without auth, that single check SHALL be generalised into a public-route test that admits both `/login` and `/about` (e.g. an `isPublicRoute` helper). The about page is intentionally a public surface; gating it behind login would be a strange UX choice for a page whose entire purpose is "tell the visitor what they are looking at."

### 6. User-menu placement: `about` goes above `preferences`/`settings`

The dropdown ordering becomes:

```
about           ← new, fully functional
preferences     ← greyed-out placeholder (foundation)
settings        ← greyed-out placeholder (foundation)
─────────────
log out
```

This ordering puts the *real* action at the top (where the user's eye lands first) and groups the disabled placeholders together. The divider stays above `log out`, matching the existing wireframe pattern.

*Alternative considered:* put `about` below `settings`. Rejected — buries the only working link in the menu.

### 7. Acknowledgements list lives in source, not config

The acknowledgements section is a short hard-coded list in the Svelte component (and the wireframe). Each entry is a name, a link, and a one-line "what we admired" hook. Examples:

- **Tripwire** (https://tripwire.eve-apps.com/) — the wormhole-mapping reference for a generation of W-space pilots; pioneered the chain-aware signature workflow.
- **Wanderer** (https://wanderer.ltd/) — modern, open-source, multi-character mapping with strong real-time semantics.
- **Anokis.info** (https://anokis.info/) — the institutional encyclopedia of W-space; the static-info source the community has trusted for years.
- **EVE Scout** (https://www.eve-scout.com/) — the Signal Cartel community effort that scouts and publicly shares the Thera and Turnur connections — open wormhole intel as a free service.

Editing this list later is a code change, not a config change. Acceptable because the list rarely changes and a config file would be over-engineering.

### 8. Legal disclaimer text is the CCP-published boilerplate, verbatim

The page renders the standard EVE third-party-developer disclaimer text without paraphrasing. The exact wording is captured in the spec so future edits to the Svelte component cannot accidentally weaken it.

### 9. UI version comes from `package.json` at build time, injected via Vite `define`

`frontend/vite.config.ts` (or the SvelteKit config) reads `package.json`'s `version` field at build time and exposes it as `import.meta.env.PUBLIC_UI_VERSION`. The about page reads that constant. This avoids:

- Hard-coding the version in two places (drift).
- Reading `package.json` at runtime (requires bundling JSON, awkward in adapter-node).

A small consequence: bumping the UI version requires a rebuild (which is true anyway; the version field is metadata for built artefacts).

### 10. A fail-closed drift test guards against accidentally-public `/api/v1` routes

The per-handler `AuthenticatedAccount` extractor (see Context, "As-built auth model") gives us typed account ids and request-data-aware Bearer/cookie resolution, at the cost of one weakness: a new `/api/v1/*` handler that forgets to name the extractor is **silently public** — there is no compiler error, because auth is opt-in. This change is the first to introduce a *deliberately*-public `/api/*` route (`/api/health`), which makes "public by accident" newly easy to confuse with "public on purpose." That is exactly when the guard should land.

We add a test (in `backend/tests/openapi_strict.rs`, sibling to the existing `all_registered_routes_are_documented`) that iterates `backend::registered_api_v1_routes()` and asserts **every** registered `/api/v1/*` route declares a `security` requirement in the OpenAPI document (the `#[utoipa::path(... security(...) ...)]` annotation the foundation already puts on authenticated handlers, e.g. `get_me`). A route registered under `/api/v1` with no `security` entry fails the test. `/api/health` is **not** in `registered_api_v1_routes()` (it is not a v1 route), so it is correctly out of scope — the guard polices the versioned business surface, not the observability carve-outs.

**Why a test and not a middleware?** A route-layer auth middleware over the `/api/v1` nest would make auth fail-*closed* by position, but at the cost of the extractor's typed account id (handlers would dig the id out of request extensions, untyped, with a runtime panic if the middleware were ever detached) and the natural home for Bearer-vs-cookie resolution. The test keeps the ergonomic extractor model and recovers the fail-closed property at CI time. See the auth-architecture discussion captured during exploration.

*Alternative considered:* convert the whole `/api/v1` tree to a `.route_layer()` auth middleware. Rejected — loses the typed `account_id`, weakens the compile-time guarantee on every handler, and the only thing it buys (fail-closed default) is recovered by this test.

## Risks / Trade-offs

- **`/api/health` exposes the backend version and commit SHA publicly** → Acceptable. Both are already inferable from public GitHub releases / tags once the repo is published; treating them as secrets buys nothing. If a future deployment needs to hide them, the endpoint can be moved behind auth as a configuration option (out of scope here).
- **DB ping per health call has a cost** → Negligible at this scale. If health probes start coming in at 100+ rps from external monitors, add a 1-second TTL cache to the handler.
- **`build.rs` running `git` is a small cross-platform footgun** → On systems without `git` installed (rare, but possible in minimal container builders), the script falls back to `"unknown"`. Tested by running the build with `PATH` stripped of `git` — the script catches the error and emits the fallback.
- **About page is now a publicly-reachable route** → The foundation's `+layout.server.ts` gates everything except `/login` (a single `isLoginRoute = url.pathname === '/login'` check, not an allowlist array — an earlier draft of this change described an array that does not exist). The change generalises that single check into a public-route test that also admits `/about`. Anyone can then read the legal disclaimer and acknowledgements without an account. That is the intended behaviour; the disclaimer's purpose is to be findable. Note the layout's `getMe` call still runs for `/about` (it runs for every route) and simply resolves to `me: null` for anonymous visitors — the about page does not *consume* `locals.me`, but the layout load is not bypassed.
- **Carve-outs in `api-contract` can multiply** → After this change, two carve-outs exist (`/auth/*`, `/api/health`). A future temptation to add a third (e.g. `/api/webhooks`) should be resisted absent the same orchestration-tooling justification. Each carve-out makes the contract slightly less uniform; the safeguard is that each MUST be a separate spec amendment, reviewed.
- **Acknowledgements are a curation surface** → Adding a project means writing copy and a link, which means taste calls. Acceptable for a 3-entry list owned by the project maintainers; if it grows past ~8 entries, consider moving to a JSON file and a small renderer.
- **No e2e test for the disclaimer text** → The wireframe + Svelte component are reviewed by humans; an automated check that "the disclaimer string contains 'CCP hf.'" is included in the about-page integration test as a guard against accidental deletion.

## Migration Plan

This change has no data migration. Deployment is:

1. Merge `eve-wormhole-mapper-foundation` to `main` and archive it (so `openspec/specs/api-contract/spec.md` exists).
2. Merge this change. The new endpoint is additive; the user-menu change is additive; the layout-redirect change only *widens* the set of routes reachable without auth (adds `/about`), changing no existing behaviour. No existing endpoint or route changes shape.
3. Verify `/api/health` returns 200 in production. Verify `/about` renders with the live version and commit.

Rollback: revert the merge commit. No DB state changes.

## Open Questions

None — the user-direct answers (repo URL, legal-text source, acknowledgements list, envelope carve-out vs. flat shape) are captured in §1, §7, §8, and `proposal.md`.
