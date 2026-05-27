## ADDED Requirements

### Requirement: Version is derived from git tags

The application version SHALL be derived at build time from git tags via `git describe --tags --always --dirty`, with the leading `v` stripped for the version string. The manifest versions (`backend/Cargo.toml`, `frontend/package.json`) SHALL NOT be the source of truth; they SHALL be frozen at a sentinel value and annotated in-file as non-authoritative.

- A build from a `v<semver>` tag SHALL produce a clean semver version (e.g. tag `v1.2.3` → `1.2.3`).
- A build from `develop` or any non-tag ref SHALL produce a semver pre-release version of the form `<last-tag>-pre.<N>.g<short-sha>` (the `git describe` default), e.g. `1.2.3-pre.4.gabc1234`.
- When no `v*` tag exists in history yet, the version SHALL fall back to `0.0.0-dev.<short-sha>`.

#### Scenario: Release tag yields clean semver
- **WHEN** a build is triggered from a `v1.2.3` git tag
- **THEN** the derived version string is `1.2.3` with no pre-release suffix

#### Scenario: Develop yields a pre-release version
- **WHEN** a build is triggered from `develop`, 4 commits past tag `v1.2.3`, at commit `abc1234`
- **THEN** the derived version string is `1.2.3-pre.4.gabc1234`

#### Scenario: No tags yet yields bootstrap version
- **WHEN** a build is triggered and no `v*` tag exists in history
- **THEN** the derived version string is `0.0.0-dev.<short-sha>`

### Requirement: Version and commit are computed outside the image and passed in as build args

Because the Docker build context contains no `.git/` directory, the version and short commit SHA SHALL be computed from the host checkout (in CI and in local build tooling) and passed to each image build as `--build-arg APP_VERSION=<version>` and `--build-arg GIT_COMMIT_SHA=<short-sha>`. The backend and frontend Dockerfiles SHALL declare matching `ARG`s and make the values available to their build steps.

#### Scenario: CI passes version and commit to both images
- **WHEN** CI builds the `erbridge-api` or `erbridge-web` image on a push
- **THEN** it passes `APP_VERSION` and `GIT_COMMIT_SHA` build args computed from the checked-out ref

#### Scenario: Image with no build args reports the documented fallback
- **WHEN** an image is built without the `APP_VERSION` / `GIT_COMMIT_SHA` build args (e.g. a raw `docker build`)
- **THEN** the build SHALL NOT fail, and version/commit fall back to their documented defaults (manifest sentinel / `"unknown"`)

### Requirement: Backend bakes the git-derived version and commit at compile time

`backend/build.rs` SHALL read the build-time `APP_VERSION` env var and, when present and non-empty, override `CARGO_PKG_VERSION` for the compiled binary (`cargo:rustc-env=CARGO_PKG_VERSION=<APP_VERSION>`), so that both `GET /api/health` and the OpenAPI document's `info.version` report the git-derived version without any handler code change. When `APP_VERSION` is unset, the manifest version SHALL be used unchanged. `build.rs` SHALL continue to bake `GIT_COMMIT_SHA` as it does today.

#### Scenario: Version available in running backend
- **WHEN** the backend binary built with `APP_VERSION=1.2.3` is started
- **THEN** `GET /api/health` returns `"version": "1.2.3"` and the OpenAPI `info.version` is also `1.2.3`

#### Scenario: No APP_VERSION falls back to manifest
- **WHEN** the backend is built with no `APP_VERSION` env var
- **THEN** `CARGO_PKG_VERSION` retains its `Cargo.toml` value

### Requirement: Frontend exposes the git-derived version and commit

The SvelteKit build SHALL receive the same `APP_VERSION` and `GIT_COMMIT_SHA`, and `vite.config.ts` SHALL inline them as `PUBLIC_UI_VERSION` (sourced from `APP_VERSION`, falling back to `package.json` when unset) and a new `PUBLIC_GIT_COMMIT`. The `/about` page SHALL display the UI commit alongside the existing UI version, API version, and API commit.

#### Scenario: Frontend version and commit available as build constants
- **WHEN** the frontend is built with `APP_VERSION` and `GIT_COMMIT_SHA` set
- **THEN** `import.meta.env.PUBLIC_UI_VERSION` equals the git-derived version and `import.meta.env.PUBLIC_GIT_COMMIT` equals the short SHA

#### Scenario: /about shows the UI commit
- **WHEN** an authenticated user views `/about`
- **THEN** the UI commit is displayed alongside the UI version, API version, and API commit

### Requirement: CI fetches tags and full history

Both `.github/workflows/backend.yml` and `frontend.yml` SHALL check out with `fetch-depth: 0` so that `git describe --tags` resolves correctly during the version-computation step.

#### Scenario: describe resolves in CI
- **WHEN** the version-computation step runs in CI
- **THEN** the checkout has fetched tags and full history, and `git describe --tags` returns a tag-relative version (not a bare SHA, except in the documented zero-tag bootstrap case)

### Requirement: The release process is documented

A `RELEASING.md` SHALL document that a release is performed by creating and pushing a `v<semver>` git tag (the manual bump step), that the manifest versions are non-authoritative, and the zero-tag bootstrap behaviour.

#### Scenario: Releasing instructions exist
- **WHEN** a developer needs to cut a release
- **THEN** `RELEASING.md` describes creating a `v<semver>` tag and what the resulting image tags and version strings will be

### Requirement: Docker tag scheme is verified, not changed

The existing image tag scheme SHALL be retained: `:<branch>` on branch pushes (`:develop`), `:v<semver>` on tag pushes, `:sha-<short>` always, and `:latest` only on `v*` release tags. This change SHALL NOT modify the tag scheme; the "semver on develop" need is met by the `APP_VERSION` baked into and reported by the develop image, not by a new image tag.

#### Scenario: Release image tags
- **WHEN** CI publishes images from a `v1.2.3` tag
- **THEN** the images carry `:v1.2.3`, `:sha-<short>`, and `:latest`

#### Scenario: Develop image tags
- **WHEN** CI publishes images from a `develop` push
- **THEN** the images carry `:develop` and `:sha-<short>` (no `:latest`), and report a `-pre.` version on `/api/health`

### Requirement: Release versions are immutable

A published release version SHALL NOT be silently changed. This is enforced at two layers:

1. **Git tag (Failure Mode A — moving the tag):** the repository SHALL have a ruleset/tag-protection rule targeting `v*` tags that restricts updates and deletions, so a `v<semver>` tag cannot be re-pointed at different code or deleted-and-recreated. This is a repository setting, not a workflow file; `RELEASING.md` SHALL document enabling it.
2. **Image tag (Failure Mode B — clobbering the artifact):** before building/pushing on a `v*` tag push, the publish job SHALL check whether the release image tag (`:v<semver>`) already exists in the registry, and SHALL fail the job with a clear message if it does. This check SHALL run only on `v*` tag pushes — the mutable `:develop` and `:sha-<short>` tags are exempt. It also covers re-running an already-published release (the re-run fails the existence check).

A pre-flight existence check has an inherent time-of-check/time-of-use window for two simultaneous identical-tag builds; GHCR's package-level immutable-tags setting (if enabled) is the race-free backstop and `RELEASING.md` SHALL mention it. Tag pushes are serial and rare, so the pre-flight check is the primary in-repo guard.

#### Scenario: Re-publishing an existing release version fails
- **WHEN** CI runs the publish job for a `v1.2.3` tag and `ghcr.io/<owner>/erbridge-api:v1.2.3` already exists
- **THEN** the job fails before pushing, with a message stating `v1.2.3` is already published and a new version must be used

#### Scenario: First publish of a new release version succeeds
- **WHEN** CI runs the publish job for a `v1.2.3` tag and that image tag does not yet exist in the registry
- **THEN** the existence check passes and the image is built and pushed

#### Scenario: Mutable tags are exempt from the existence check
- **WHEN** CI publishes from a `develop` push (re-pushing the `:develop` tag)
- **THEN** the existence check does not run and the push proceeds normally

#### Scenario: Moving a release git tag is blocked
- **WHEN** a developer attempts to force-update or delete an existing `v1.2.3` git tag
- **THEN** the repository tag-protection ruleset rejects the operation
