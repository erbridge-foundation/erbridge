## Context

The application ships as two Docker images (`erbridge-api`, `erbridge-web`) published to GHCR by `.github/workflows/{backend,frontend}.yml`. Nobody consumes the backend as a crate or the frontend as an npm package — **the image is the product**, so the manifest version strings (`Cargo.toml` `0.1.0`, `package.json` `0.0.1`) do no real work and have drifted into meaninglessness.

Much of the machinery the original proposal called for already exists:

- `backend/build.rs` resolves `GIT_COMMIT_SHA` (explicit env override → `git rev-parse --short HEAD` → `"unknown"`), and `health.rs` returns it.
- `backend/src/openapi.rs` and `health.rs` both read `CARGO_PKG_VERSION`.
- `frontend/vite.config.ts` inlines `package.json` version as `PUBLIC_UI_VERSION`; `/about` shows UI version + API version + API commit.
- CI tags images via `docker/metadata-action`: branch name, tag name, `sha-<short>`, and `:latest` gated on `startsWith(github.ref, 'refs/tags/v')`.

What is broken: the build.rs `GIT_COMMIT_SHA` **override hook is never fed** in Docker (the build context has no `.git/` and no build-arg is passed), so published images report `commit: "unknown"`; and there is **no source of truth** for the version number.

## Goals / Non-Goals

**Goals:**
- One version number for the whole repo, derived automatically from git tags.
- Real `commit` and `version` in published backend images and the frontend UI.
- Frontend gains a commit string (it has none today).
- A documented, manual release process (tagging IS the bump).

**Non-Goals:**
- Automatic semver bumping or changelog generation.
- Changing the existing Docker tag scheme (it is correct; only verified).
- Multiple simultaneous release trains.
- Removing `:latest` (kept; already gated to release tags only).

## Decisions

### D1: Git tags are the single source of truth (`git describe --tags`)
Version is computed at build time as `git describe --tags --always --dirty`:
- On a `v1.2.3` tag → `1.2.3` (the leading `v` is stripped for the version string).
- On `develop` → `1.2.3-pre.4.gabc1234` (4 commits past `v1.2.3`, at `abc1234`) — valid semver pre-release, monotonic, automatic.

Manifest versions are **frozen** (set to a sentinel like `0.0.0`) and annotated in-file as non-authoritative ("derived from git tags; see RELEASING.md"). This is standard for a deployed app (vs a published library, where the manifest must hold the truth because consumers resolve it).

### D2: `build.rs` overrides `CARGO_PKG_VERSION` (not a separate `APP_VERSION` runtime env)
CI/justfile passes `APP_VERSION` as a build-time env/arg. `build.rs` emits `cargo:rustc-env=CARGO_PKG_VERSION=$APP_VERSION` (and `rerun-if-env-changed=APP_VERSION`). Both `health.rs` and `openapi.rs` keep reading `CARGO_PKG_VERSION` unchanged — so `/api/health.version` **and** the OpenAPI `info.version` update together and stay honest, with zero handler code change. If `APP_VERSION` is unset (plain local `cargo build`), the real `Cargo.toml` value is used as today.

### D3: Frontend uses the same git source as the backend
The SvelteKit build receives `APP_VERSION` + `GIT_COMMIT_SHA` as build args/env. `vite.config.ts` inlines them as `PUBLIC_UI_VERSION` (sourced from `APP_VERSION`, falling back to `package.json` locally) and a new `PUBLIC_GIT_COMMIT`. One version across the whole repo; `/about` shows the UI commit next to the existing rows.

### D4: Version + commit are computed OUTSIDE the image and passed in
Because the Dockerfiles copy source without `.git/`, `git describe`/`rev-parse` cannot run inside the build. CI (and the local `just docker-build*` targets) compute both values from the host checkout and pass them as `--build-arg`. The Dockerfiles add `ARG APP_VERSION` / `ARG GIT_COMMIT_SHA` and promote them to `ENV` for the build step. This mirrors the existing (intended) `GIT_COMMIT_SHA` model.

### D5: Tag scheme unchanged
`:develop` (branch), `:v1.2.3` (tag), `:sha-<short>` (always), `:latest` (release only). The "want semver on develop" desire is satisfied by the `APP_VERSION` baked *into* the image and reported on `/api/health` — not by adding a new image tag. `:latest` stays; its release-only gate already prevents it tracking develop.

## Risks / Trade-offs

- **R1 — `git describe` needs tags + history in CI.** `actions/checkout@v4` defaults to `fetch-depth: 1` and does not fetch tags, so `git describe --tags` would fail/return a bare SHA. **Mitigation:** set `fetch-depth: 0` on the checkout in both workflows (a task, and the single most common failure mode of this setup).
- **R2 — zero tags today.** The repo has 0 tags. With no tag, `git describe --tags --always` returns a bare SHA — not semver. **Mitigation:** the version-computation step defines an explicit pre-bootstrap fallback `0.0.0-dev.<sha>` until the first `v*` tag exists; document it in RELEASING.md.
- **R3 — overriding `CARGO_PKG_VERSION` is mild "magic".** `env!("CARGO_PKG_VERSION")` reads a value build.rs overrode, not literally `Cargo.toml`. **Mitigation:** comment in build.rs and the frozen-manifest annotation make the indirection explicit; accepted because it keeps handler + OpenAPI code untouched and honest.
- **R4 — local images still report sentinel unless `just` passes args.** **Mitigation:** update `just docker-build*` to compute and pass the build args (and document that a raw `docker build ./backend` will report the fallback).

## Open Questions

(None outstanding — resolved during exploration: source = git tags; pre-release format = `git describe` default; `:latest` kept release-only; frontend uses git source; backend overrides `CARGO_PKG_VERSION`.)
