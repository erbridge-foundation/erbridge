## Context

The foundation change (`eve-wormhole-mapper-foundation`) ships the app's auth, identity, and characters surfaces and bakes in the `api-contract` rule that every `/api/*` response uses a `{ data, meta? }` success envelope (with a single carve-out for `/auth/*`). It does not include an about page or any health endpoint.

Three things now want an answer in the same change because they are tightly coupled:

1. **Where does the API version come from?** Hard-coding it on the frontend means the displayed version can lag the running binary. A small `/health` endpoint that returns `CARGO_PKG_VERSION` is the obvious source of truth.
2. **Where do the legal disclaimer and acknowledgements live?** EVE third-party developers must display CCP's standard disclaimer somewhere the user can find. Burying it in CLAUDE.md is wrong; surfacing it in the user-menu under `about` is the conventional place.
3. **How do orchestration tools (Traefik, future k8s probes, uptime monitors) probe liveness?** The same endpoint that drives the about page can serve their needs if it returns a shape that's friendly to shallow parsing.

Goals for this change are therefore both UX (a polished about page) and infra (a real health endpoint). Both are small, but the interaction with the api-contract envelope rule needs a deliberate decision, captured below.

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

A small `backend/build.rs` runs `git rev-parse --short HEAD` and emits `cargo:rustc-env=GIT_COMMIT_SHA=<sha>`. The handler reads it via `env!("GIT_COMMIT_SHA")`. Two fallback paths matter:

- **Source distribution without `.git/`** (e.g. someone unpacks a tarball): `build.rs` SHALL not panic. It MUST emit `cargo:rustc-env=GIT_COMMIT_SHA=unknown` and exit normally. The handler's behaviour is identical; `/api/health` just returns `"commit": "unknown"`.
- **Docker build context**: by default, Docker copies `.git/` if the user does not `.dockerignore` it. The backend Dockerfile (foundation §3) does not exclude `.git/`, so production builds will have it. If a future change adds `.git/` to `.dockerignore`, builds will silently fall back to `"unknown"` — acceptable, but worth noting.

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
- Does NOT use `+layout.server.ts`'s `locals.me` — about must work for unauthenticated users too (you can be redirected to `/login`, but the page itself does not require an account). The auth gate is at the route layer per foundation §11.

A wrinkle: foundation's `+layout.server.ts` redirects to `/login` on 401 except when the route is `/login`. To make `/about` reachable without auth, the layout's redirect rule SHALL be relaxed to include `/about` in its allowlist (alongside `/login`). The about page is intentionally a public surface; gating it behind login would be a strange UX choice for a page whose entire purpose is "tell the visitor what they are looking at."

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

Editing this list later is a code change, not a config change. Acceptable because the list rarely changes and a config file would be over-engineering.

### 8. Legal disclaimer text is the CCP-published boilerplate, verbatim

The page renders the standard EVE third-party-developer disclaimer text without paraphrasing. The exact wording is captured in the spec so future edits to the Svelte component cannot accidentally weaken it.

### 9. UI version comes from `package.json` at build time, injected via Vite `define`

`frontend/vite.config.ts` (or the SvelteKit config) reads `package.json`'s `version` field at build time and exposes it as `import.meta.env.PUBLIC_UI_VERSION`. The about page reads that constant. This avoids:

- Hard-coding the version in two places (drift).
- Reading `package.json` at runtime (requires bundling JSON, awkward in adapter-node).

A small consequence: bumping the UI version requires a rebuild (which is true anyway; the version field is metadata for built artefacts).

## Risks / Trade-offs

- **`/api/health` exposes the backend version and commit SHA publicly** → Acceptable. Both are already inferable from public GitHub releases / tags once the repo is published; treating them as secrets buys nothing. If a future deployment needs to hide them, the endpoint can be moved behind auth as a configuration option (out of scope here).
- **DB ping per health call has a cost** → Negligible at this scale. If health probes start coming in at 100+ rps from external monitors, add a 1-second TTL cache to the handler.
- **`build.rs` running `git` is a small cross-platform footgun** → On systems without `git` installed (rare, but possible in minimal container builders), the script falls back to `"unknown"`. Tested by running the build with `PATH` stripped of `git` — the script catches the error and emits the fallback.
- **About page is now a publicly-reachable route** → The layout redirect allowlist grows from `["/login"]` to `["/login", "/about"]`. Anyone can read the legal disclaimer and acknowledgements without an account. That is the intended behaviour; the disclaimer's purpose is to be findable.
- **Carve-outs in `api-contract` can multiply** → After this change, two carve-outs exist (`/auth/*`, `/api/health`). A future temptation to add a third (e.g. `/api/webhooks`) should be resisted absent the same orchestration-tooling justification. Each carve-out makes the contract slightly less uniform; the safeguard is that each MUST be a separate spec amendment, reviewed.
- **Acknowledgements are a curation surface** → Adding a project means writing copy and a link, which means taste calls. Acceptable for a 3-entry list owned by the project maintainers; if it grows past ~8 entries, consider moving to a JSON file and a small renderer.
- **No e2e test for the disclaimer text** → The wireframe + Svelte component are reviewed by humans; an automated check that "the disclaimer string contains 'CCP hf.'" is included in the about-page integration test as a guard against accidental deletion.

## Migration Plan

This change has no data migration. Deployment is:

1. Merge `eve-wormhole-mapper-foundation` to `main` and archive it (so `openspec/specs/api-contract/spec.md` exists).
2. Merge this change. The new endpoint is additive; the user-menu change is additive; the layout-redirect allowlist change is additive. No existing endpoint or route changes shape.
3. Verify `/api/health` returns 200 in production. Verify `/about` renders with the live version and commit.

Rollback: revert the merge commit. No DB state changes.

## Open Questions

None — the user-direct answers (repo URL, legal-text source, acknowledgements list, envelope carve-out vs. flat shape) are captured in §1, §7, §8, and `proposal.md`.
