## Why

The foundation change ships a working app but gives the user nowhere to see what they are running, where the source lives, or what the legal / acknowledgement footprint is. EVE third-party tooling has a non-negotiable legal disclaimer obligation (EVE Online and the EVE logo are CCP hf. trademarks; third-party tools must say so), and an open-source project benefits from a "where does this code live and what inspired it" page. This change adds the minimum viable about page plus the health endpoint it depends on, both small and well-bounded so they can land as a single follow-up without inflating the foundation.

## What Changes

- New `GET /api/health` endpoint on the backend, **public (no auth)** and **not enveloped** — returns a flat JSON document `{ status, version, commit, components: [{ name, status }] }`. Overall `status = "ok"` iff every component status is `"ok"`, else `"degraded"`. The initial `components` array contains one entry, `db`, populated by pinging Postgres in the handler.
- **BREAKING (spec, not wire)**: amend the `api-contract` capability to carve `/api/health` out of the success-envelope rule, parallel to the existing `/auth/*` carve-out. No existing endpoint changes shape — this only documents the exception used by the new endpoint.
- New `/about` route in the SvelteKit frontend, server-rendered (`+page.server.ts` fetches `/api/health`). Shows:
  - the brand mark and a short tagline,
  - the **UI version** (from `frontend/package.json`, build-time inlined),
  - the **API version** and **commit SHA** (fetched from `/api/health`),
  - a **GitHub** link to `https://github.com/erbridge-foundation/erbridge`,
  - a **CCP / EVE Online legal disclaimer** (standard third-party developer boilerplate),
  - an **acknowledgements** section thanking Tripwire, Wanderer, and Anokis.info with one-line nods to what each contributed to wormhole mapping culture.
- A new `about` link in the user-menu dropdown, placed **above** the existing `preferences` / `settings` placeholders, with the divider remaining above `log out`. The link is fully functional (unlike the placeholders).
- A new wireframe (`wireframes/about.html`) that locks in the visual contract before the SvelteKit work begins, consistent with the foundation's wireframe-first workflow.

## Capabilities

### New Capabilities

- `api-health`: defines `GET /api/health` — its response shape, public access, component-aggregation rule, and the fact that it is public by omitting the per-handler `AuthenticatedAccount` extractor (this codebase has no auth-middleware router split), registered alongside `/api/openapi.json` and `/api/docs`. Also adds a fail-closed requirement that every `/api/v1` route declares authentication.
- `about-page`: defines the `/about` route — its content sections (version info, repo link, legal disclaimer, acknowledgements), how it is reached from the user-menu dropdown, and its degraded-state behaviour when `/api/health` is unreachable.

### Modified Capabilities

- `api-contract`: add a single carve-out clause to the existing "JSON response envelope for /api/*" requirement, exempting `/api/health` (parallel in form and intent to the existing `/auth/*` carve-out). All other `/api/*` endpoints remain enveloped.

## Impact

- **Backend**: a new handler module `backend/src/handlers/health.rs`, registered in `backend/src/lib.rs::build_router` next to the SwaggerUi merge for `/api/openapi.json` (auth is per-handler via the `AuthenticatedAccount` extractor — the route is public by omitting it, not by living on a separate router branch). A fail-closed auth-coverage test (`backend/tests/openapi_strict.rs`) asserts every `/api/v1` route declares `security`. New dependency on `git2` is **not** introduced — the commit SHA is captured at compile time via a `build.rs` that runs `git rev-parse --short HEAD` and exposes the result via `env!("GIT_COMMIT_SHA")` (falls back to `"unknown"` in source distributions where `.git/` is absent). One new unit test (overall-status aggregation rule) and one integration test (`backend/tests/health.rs`) hitting the route with a healthy DB. One HURL test (`backend/tests/hurl/health.hurl`) asserting the flat shape.
- **Frontend**: a new route `frontend/src/routes/about/+page.svelte` and `+page.server.ts`. The user-menu dropdown component (`frontend/src/lib/components/UserMenu.svelte`) gains one new menu item. The UI version is sourced from `frontend/package.json` via Vite's `import.meta.env` (defined in `vite.config.ts` from the `version` field at build time).
- **OpenAPI**: `/api/health` is annotated with `#[utoipa::path]` so it appears in `/api/openapi.json` and `/api/docs`. Its response schema (`HealthResponse`) is a top-level component, not wrapped in `ApiResponse<T>` — the strict-drift test (foundation §2d.6) must learn that this one route is exempt from the envelope shape, or the route's annotation can simply declare the flat schema directly (preferred).
- **Strict-drift test**: foundation `backend/tests/openapi_strict.rs` validates every documented route's actual response against its declared schema. Because `/api/health`'s declared schema is the flat `HealthResponse`, no change to the test harness is needed — it already validates whatever shape the annotation declares.
- **Dependency on foundation**: this change cannot be archived before `eve-wormhole-mapper-foundation` is archived, because it modifies the `api-contract` spec that foundation introduces. Implementation order: foundation merges → foundation archives → this change implements + archives.
