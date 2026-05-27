## Why

The version-handling machinery is *mostly already present* but disconnected, and the version number itself is meaningless:

- `backend/build.rs` resolves a short git SHA into `GIT_COMMIT_SHA`, and `health.rs` returns it — **but** the Dockerfile copies source without `.git/` and neither the Dockerfile nor CI passes the `GIT_COMMIT_SHA` build-arg the build.rs explicitly supports. So **every published image reports `commit: "unknown"`**.
- `version` on `/api/health` is `CARGO_PKG_VERSION` = `0.1.0`, and the frontend's `PUBLIC_UI_VERSION` is `package.json` = `0.0.1`. Both are hand-edited, unrelated to each other, and have never been bumped — so the version strings are meaningless.
- The frontend surfaces a UI version on `/about` but has **no commit** concept of its own.

The fix is not to build new machinery — it is to (a) establish a single source of truth for the version, (b) connect the existing injection hooks so real values reach built images, and (c) document the release process so the manual bump step is clear.

## What Changes

- **Single source of truth = git tags.** `git describe --tags` derives the version: clean semver (`1.2.3`) on `v*` tags, an automatic pre-release string (`1.2.3-pre.<N>.g<sha>`) on `develop`. The manifest versions (`Cargo.toml`, `package.json`) are frozen and annotated as non-authoritative.
- **Backend:** `build.rs` overrides `CARGO_PKG_VERSION` from a build-time `APP_VERSION` (so both `/api/health` and the OpenAPI `info.version` update together) and bakes `GIT_COMMIT_SHA`. The Dockerfile gains `ARG APP_VERSION` / `ARG GIT_COMMIT_SHA`; CI and local `just docker-build` compute and pass them.
- **Frontend:** the SvelteKit build receives the same git-derived `APP_VERSION` and `GIT_COMMIT_SHA` as build args; vite inlines them as `PUBLIC_UI_VERSION` and `PUBLIC_GIT_COMMIT`. `/about` shows the UI commit alongside the existing UI/API versions.
- **CI:** `actions/checkout` fetches tags + full depth (`fetch-depth: 0`) so `git describe` works; a step computes `APP_VERSION` + `GIT_COMMIT_SHA` once and passes them as build-args to both image builds.
- **Release immutability:** on `v*` tag pushes the publish job refuses to re-publish an already-existing `:v<semver>` image tag (fails hard); the `:develop`/`:sha-` mutable tags are exempt. A `v*` git-tag-protection ruleset (restrict updates + deletions) prevents the underlying tag from being moved. Tags `:develop`, `:sha-`, `:latest` are unchanged.
- **Docs:** add `RELEASING.md` documenting the manual bump = create a `v<semver>` git tag, plus the zero-tag bootstrap behaviour.

## Capabilities

### New Capabilities

- `release-versioning`: Git-tag-derived version scheme, build-time injection of version + commit into backend and frontend, Docker build-arg wiring, CI tag-fetch + version computation, release-version immutability (in-CI existence check + git tag protection), and a documented release process.

### Modified Capabilities

- `api-health`: The `version` field is redefined from "the backend `CARGO_PKG_VERSION` at compile time" to "the git-tag-derived application version baked in at compile time". `commit` is unchanged in contract but the build wiring is fixed so it stops always being `"unknown"` in published images.

## Impact

- Backend: `build.rs` extended (override `CARGO_PKG_VERSION`, keep `GIT_COMMIT_SHA`); `Dockerfile` gains build args; no handler code change (`health.rs`/`openapi.rs` already read `CARGO_PKG_VERSION`).
- Frontend: `vite.config.ts` gains `PUBLIC_GIT_COMMIT` and sources `PUBLIC_UI_VERSION` from `APP_VERSION` env (falling back to `package.json` locally); `Dockerfile` gains build args; `/about` + `app.d.ts` updated.
- CI: both `.github/workflows/*.yml` gain `fetch-depth: 0` and a version-computation step feeding `build-args`.
- Local: `just docker-build*` targets compute and pass the same build args.
- Specs: `api-health` modified (above); `project-infrastructure` may reference the release process.
- No change to the existing Docker tag scheme itself (`:develop`, `:v*`, `:sha-`, `:latest`-on-release): it is correct as-is and is only verified, not modified.
