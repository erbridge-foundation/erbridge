# Releasing E-R Bridge

The product is two Docker images published to GHCR:

- `ghcr.io/erbridge-foundation/erbridge-api`
- `ghcr.io/erbridge-foundation/erbridge-web`

Nobody consumes the backend as a crate or the frontend as an npm package, so the
manifest versions are **not** the source of truth.

## The version is derived from git tags

The application version is computed at build time with:

```sh
git describe --tags --always --dirty   # leading "v" stripped for the version string
```

- A build from a `v<semver>` tag → clean semver, e.g. tag `v1.2.3` → `1.2.3`.
- A build from `develop` (or any non-tag ref) → a semver pre-release of the form
  `<last-tag>-pre.<N>.g<short-sha>`, e.g. `1.2.3-pre.4.gabc1234` (the `git describe`
  default). This is monotonic and automatic — no manual bump on develop.
- **Zero-tag bootstrap:** when no `v*` tag exists yet, `git describe --tags --always`
  returns a bare SHA, which is not semver. CI and the local build tooling therefore
  fall back to `0.0.0-dev.<short-sha>` until the first `v*` tag exists.

The version + short commit SHA are computed **outside** the Docker build (the build
context has no `.git/`) and passed in as build args:

- `APP_VERSION` — the git-derived version (leading `v` stripped).
- `GIT_COMMIT_SHA` — `git rev-parse --short HEAD`.

The backend `build.rs` overrides `CARGO_PKG_VERSION` from `APP_VERSION`, so both
`GET /api/health` and the OpenAPI `info.version` report it. The frontend inlines
`APP_VERSION` as `PUBLIC_UI_VERSION` and `GIT_COMMIT_SHA` as `PUBLIC_GIT_COMMIT`
(shown on `/about`).

### Manifest versions are frozen

`backend/Cargo.toml` and `frontend/package.json` are pinned at the sentinel `0.0.0`
and annotated in-file as non-authoritative. They are only used by a build with no
`APP_VERSION` (a raw `cargo build` / `docker build` with no build arg), which reports
the sentinel `0.0.0` (backend) / `0.0.0` (frontend `PUBLIC_UI_VERSION`) and
`commit: "unknown"`. **Do not** hand-bump them — bump by tagging.

## What publishes an image

CI publishes images for two refs only:

| Push | check job | image published? | tags |
| --- | --- | --- | --- |
| `develop` branch | ✅ | ✅ | `:develop`, `:sha-<short>` (staging line) |
| `v<semver>` tag | ✅ | ✅ | `:v<semver>`, `:sha-<short>`, `:latest` (production) |
| `main` branch | ✅ | ❌ **check-only** | — |
| pull request | ✅ | ❌ | — |

`main` is the branch releases are cut **from** — it is not itself a deployed
environment, so a `main` push runs the full check but publishes nothing. `:latest`
moves only on a `v*` tag push, so it always points at the newest production release.

## Cutting a release

A release **is** the tag. There is no separate version-bump commit.

```sh
git checkout main
git pull
git tag v1.2.3        # the manual "bump"
git push origin v1.2.3
```

CI (`.github/workflows/{backend,frontend}.yml`) then publishes each image with:

- `:v1.2.3` (the release tag)
- `:sha-<short>` (always)
- `:latest` (release tags only)

The published `erbridge-api` reports `version: 1.2.3` + the real commit on
`/api/health`; `erbridge-web` shows UI version `1.2.3` + the real UI commit on
`/about`.

## Release immutability

A published release version must never silently change. Two layers enforce this:

1. **Git tag — moving the tag (Failure Mode A).** A repository ruleset, **"Protect
   release tags"** (id `16932417`), targets `v*` tags and restricts **updates** and
   **deletions** (repo-admin bypass only). A `v<semver>` tag cannot be re-pointed at
   different code or deleted-and-recreated. This is a GitHub repository setting, not
   a workflow file.

2. **Image tag — clobbering the artifact (Failure Mode B).** On `v*` tag pushes only,
   each publish job runs a pre-flight check: if `:v<semver>` already exists in GHCR
   it **fails the job before pushing**, with a message saying the version is already
   published and a new version is required. This also catches re-running an
   already-published release. The mutable `:develop` and `:sha-<short>` tags are
   exempt.

   The pre-flight check has an inherent time-of-check/time-of-use window if two
   identical-tag builds run simultaneously. Tag pushes are serial and rare, so the
   check is the primary guard; **GHCR package-level immutable tags** (enabled per
   package in the GHCR UI) are the race-free backstop and may optionally be turned on.

## Local image builds

`just docker-build-backend` / `docker-build-frontend` compute `APP_VERSION` +
`GIT_COMMIT_SHA` from the local checkout and pass them as `--build-arg`, so locally
built images report the same git-derived version as CI.

A raw `docker build ./backend` (no build args) does **not** fail — it reports the
documented fallbacks (`version: 0.0.0`, `commit: "unknown"`).

## Dev compose

`docker-compose.dev.yml` does not pass these build args, so dev images report the
documented fallbacks (`0.0.0` / `unknown`). This is acceptable for dev — the version
is only meaningful for published release/develop images.
