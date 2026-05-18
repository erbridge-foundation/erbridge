## 0. Required skills

Before implementing tasks, the implementer MUST invoke the skill matching the area being worked on. Each skill defines mandatory architecture, structure, and convention rules.

| Working on… | Skill | Invoke when |
|---|---|---|
| Anything under `backend/` (sections 2, 4) | `rust-rest-api` | Before writing the first line of Rust in this session |
| Anything under `frontend/` (sections 5, 6) | `sveltekit-node` | Before writing the first line of Svelte / TypeScript in `frontend/` in this session. §3 wireframe is plain HTML and does NOT require this skill, but it must be approved before §5/§6 begin. |

## 0a. Prerequisite

This change depends on `eve-wormhole-mapper-foundation` being archived first. The `api-contract` MODIFIED requirement targets a spec that only exists in `openspec/specs/api-contract/spec.md` after the foundation is archived. Do NOT begin implementation while foundation is still an in-flight change.

## 1. Backend: build-time commit SHA capture

- [ ] 1.1 Add `backend/build.rs` that runs `git rev-parse --short HEAD`, captures stdout, and emits `cargo:rustc-env=GIT_COMMIT_SHA=<sha>`. If the command fails for any reason (no `git` on PATH, no `.git/` directory, non-zero exit), the script SHALL emit `cargo:rustc-env=GIT_COMMIT_SHA=unknown` and exit 0. The script SHALL also emit `cargo:rerun-if-changed=.git/HEAD` and `cargo:rerun-if-changed=.git/refs/` so the SHA is re-baked when the working tree's HEAD moves.
- [ ] 1.2 Add a unit test (in `backend/build.rs` is not testable directly; add the test to `backend/src/handlers/health.rs` instead): assert `env!("GIT_COMMIT_SHA")` resolves to a non-empty string at compile time (this fails to compile if `build.rs` did not run).
- [ ] 1.3 Verify locally: `cargo build` from inside the repo with `.git/` present produces a binary whose health handler reports a real SHA; `cargo build` from a tarball-extracted copy without `.git/` produces a binary that reports `"unknown"`. Document the verification method in the commit message.

## 2. Backend: /api/health endpoint

- [ ] 2.1 Add `backend/src/handlers/health.rs` per the `rust-rest-api` skill's handler-layer rules. Define a `HealthResponse` struct (in `backend/src/dto/health.rs` per the skill's DTO layering rule) with `#[derive(Serialize, utoipa::ToSchema)]` and fields `status: HealthStatus`, `version: String`, `commit: String`, `components: Vec<ComponentHealth>`. Define `HealthStatus` and `ComponentStatus` as serde-string-enum types (`#[serde(rename_all = "snake_case")]`) with variants `Ok` and `Degraded`. Define `ComponentHealth { name: String, status: ComponentStatus }`.
- [ ] 2.2 Implement the service-layer function `backend/src/services/health.rs::check(pool: &PgPool) -> HealthSnapshot` that runs `sqlx::query!("SELECT 1 AS one").fetch_one(pool).await` and returns a `HealthSnapshot { db: ComponentStatus }`. Map `Ok(_)` to `Ok`, `Err(_)` to `Degraded`. Do NOT propagate the error — degraded is the success signal, the endpoint returns 200 in both cases.
- [ ] 2.3 Implement the handler function `backend/src/handlers/health.rs::get_health(State(state): State<AppState>) -> Json<HealthResponse>` that calls `services::health::check(&state.pool).await`, builds the `components` array (`[{ name: "db", status: snapshot.db }]`), derives the overall status (`Ok` iff every component is `Ok`, else `Degraded`), and returns `Json(HealthResponse { status, version: env!("CARGO_PKG_VERSION").into(), commit: env!("GIT_COMMIT_SHA").into(), components })`. The function MUST NOT touch the session store or any auth machinery.
- [ ] 2.4 Add a unit test in `backend/src/services/health.rs` for the overall-status aggregation rule: pass a snapshot with all-`Ok` components and assert the overall is `Ok`; pass one with at least one `Degraded` and assert the overall is `Degraded`. (Pure-function test; no DB needed.)
- [ ] 2.5 Mount the route in `backend/src/main.rs`: add `GET /api/health` to the **public** router branch alongside `/api/openapi.json` and `/api/docs`, NOT behind the `AuthenticatedAccount` middleware. Document with a one-line comment that this is the documented carve-out from the `api-contract` envelope rule.
- [ ] 2.6 Annotate the handler with `#[utoipa::path(get, path = "/api/health", responses((status = 200, description = "Health snapshot", body = HealthResponse)))]`. Add `HealthResponse`, `HealthStatus`, `ComponentHealth`, and `ComponentStatus` to the `#[derive(utoipa::OpenApi)]` collector in `backend/src/openapi.rs` so they appear in `/api/openapi.json`. Confirm via `curl $APP_URL/api/openapi.json | jq '.paths."/api/health"'` that the route is documented and its response schema is the flat `HealthResponse` (NOT wrapped in `ApiResponse<T>`).
- [ ] 2.7 Add `backend/tests/health.rs` (integration test using `#[sqlx::test]` per the `rust-rest-api` skill): build the router, hit `GET /api/health`, assert HTTP 200, parse the body as `HealthResponse`, assert `version` is non-empty, `commit` is non-empty, `components` contains exactly one element with `name = "db"` and `status = Ok`, and the overall `status = Ok`.
- [ ] 2.8 Add `backend/tests/hurl/health.hurl` per the `rust-rest-api` skill's HURL convention. Assertions: `HTTP 200`; `jsonpath "$.status" == "ok"`; `jsonpath "$.version" exists`; `jsonpath "$.commit" exists`; `jsonpath "$.components" count == 1`; `jsonpath "$.components[0].name" == "db"`; `jsonpath "$.components[0].status" == "ok"`; **and** `jsonpath "$.data" not exists` (proves the response is NOT enveloped).
- [ ] 2.9 Update `backend/tests/openapi_strict.rs` (foundation §2d.6) to include `/api/health` in its `cases` list, validating the 200 response against the flat `HealthResponse` schema. No special-casing needed if the test harness already validates each route against whatever schema the annotation declares.

## 3. Wireframe (author and approve BEFORE frontend implementation)

- [ ] 3.1 Author `openspec/changes/add-about-page/wireframes/about.html` matching the foundation wireframes' visual conventions (inlined design tokens, JetBrains Mono, full GlobalNav on top). Page sections, top to bottom:
  1. **Header**: brand mark + brief tagline ("Wormhole Mapper for EVE Online").
  2. **Versions**: two stacked rows showing `UI version: <ui_version>` and `API version: <api_version> · <commit_sha>` (or `API: unreachable` when health is down — include both states in the wireframe, the second labelled "Degraded state example").
  3. **Source code**: a single line "Source on GitHub →" linking to https://github.com/erbridge-foundation/erbridge.
  4. **Legal**: the verbatim CCP disclaimer text from the `about-page` spec, rendered as muted paragraph text in `--slate-400`.
  5. **Acknowledgements**: heading `ACKNOWLEDGEMENTS` in `--slate-500` uppercase; three entries (Tripwire, Wanderer, Anokis.info) each as a name + link + one-line description.
- [ ] 3.2 Author `openspec/changes/add-about-page/wireframes/user-menu.html`: render the foundation's user-menu wireframe with the new `about` link added as the first item (above `preferences` and `settings`). Use this to lock in the menu's new ordering before §6 begins.
- [ ] 3.3 The user opens both wireframes in a browser and signs off. Cosmetic tweaks land in the wireframe; spec-affecting changes (e.g. moving the disclaimer above the versions) require updating `specs/about-page/spec.md` first.

## 4. Frontend: vite/SvelteKit version injection

- [ ] 4.1 In `frontend/vite.config.ts` (or `svelte.config.js`, whichever owns Vite config in this repo per the `sveltekit-node` skill), read `package.json`'s `version` field at build time and define `import.meta.env.PUBLIC_UI_VERSION` via Vite's `define` option (e.g. `define: { 'import.meta.env.PUBLIC_UI_VERSION': JSON.stringify(pkg.version) }`). Add the corresponding type declaration to `frontend/src/app.d.ts` so TypeScript knows the constant exists.
- [ ] 4.2 Verify locally: `pnpm dev` and `pnpm build` both succeed; opening any page and logging `import.meta.env.PUBLIC_UI_VERSION` in the browser console prints the value from `package.json`.

## 5. Frontend: /about route

- [ ] 5.1 Update `frontend/src/routes/+layout.server.ts` (foundation §4.5): expand the layout's redirect-to-`/login` allowlist from `["/login"]` to `["/login", "/about"]`. The `/about` route SHALL render without an authenticated session.
- [ ] 5.2 Implement `frontend/src/routes/about/+page.server.ts`: server-side `fetch('/api/health')` (forwarding no cookies — health is public). On 200, return `{ health: <parsed body> }`. On any error (network failure, non-2xx), return `{ health: null, healthError: { message: <stringified error> } }`. The page MUST render in both cases.
- [ ] 5.3 Implement `frontend/src/routes/about/+page.svelte` matching `wireframes/about.html`. Use Svelte 5 syntax per the `sveltekit-node` skill (`$props`, `$derived` where appropriate). Sections:
  - Header (brand mark + tagline).
  - Versions block: render `import.meta.env.PUBLIC_UI_VERSION`; render `{data.health.version} · {data.health.commit}` when `data.health` is set, or `API: unreachable` when `data.health` is null.
  - GitHub link with `target="_blank"` and `rel="noopener noreferrer"`.
  - Legal disclaimer paragraph containing the verbatim text from `specs/about-page/spec.md` (the string MUST contain `"CCP hf."`).
  - Acknowledgements list of three entries (Tripwire, Wanderer, Anokis.info), each linking with `target="_blank"` and `rel="noopener noreferrer"`.

## 6. Frontend: user-menu link

- [ ] 6.1 Update `frontend/src/lib/components/UserMenu.svelte` (foundation §4.8): insert a new menu item `about` as the **first** item, with `href="/about"` and no `aria-disabled`. The existing items (`preferences`, `settings`, divider, `log out`) shift down by one position. The divider stays above `log out`.

## 7. Verification

- [ ] 7.1 `curl $APP_URL/api/health` (unauthenticated) returns HTTP 200 with a body whose top-level keys are exactly `status`, `version`, `commit`, `components` (no `data`). `status = "ok"`, `version` matches `CARGO_PKG_VERSION`, `commit` matches `git rev-parse --short HEAD`.
- [ ] 7.2 Stop the Postgres container (`docker compose stop postgres`) and re-curl `$APP_URL/api/health`. Response is HTTP 200 with `status = "degraded"` and `components[0] = { name: "db", status: "degraded" }`. Restart Postgres and confirm the next probe is back to `"ok"`.
- [ ] 7.3 `curl $APP_URL/api/openapi.json | jq '.paths."/api/health"'` shows the documented route with response schema = `HealthResponse` (no envelope wrapping).
- [ ] 7.4 The backend `cargo test` suite passes, including the new unit test (§2.4), the new integration test (§2.7), the HURL test (§2.8), and the strict-drift test (§2.9).
- [ ] 7.5 Navigate to `/about` in a browser **without** logging in. The page renders the brand mark, UI version, API version, commit, GitHub link, legal disclaimer, and acknowledgements. Confirm the GitHub link opens a new tab to `https://github.com/erbridge-foundation/erbridge`. Confirm all three acknowledgement links (Tripwire, Wanderer, Anokis.info) open in new tabs.
- [ ] 7.6 Stop the backend container and reload `/about`. The page still renders; the API version row reads `API: unreachable`; all other sections are unchanged.
- [ ] 7.7 Log in. Open the user-menu dropdown. The `about` link is the first item, enabled, with `href="/about"`. Click it; the page loads and the layout still renders (no redirect to `/login`). Click `log out`; the redirect to `/login` works.
- [ ] 7.8 Open `wireframes/about.html` and the live `/about` side-by-side. Layouts SHALL be visually equivalent; tune spacing/colours until they match. Document deliberate deviations as comments in the Svelte component.
- [ ] 7.9 **Pre-archival**: move `openspec/changes/add-about-page/wireframes/about.html` to `frontend/wireframes/about.html` (alongside the foundation wireframes relocated by foundation §7.29) so the wireframe survives archival as a tracked, durable artefact. The `user-menu.html` wireframe in this change can be deleted — its purpose was to lock in the menu's new ordering before §6, and the live component is now authoritative.
