## Why

Currently the backend and frontend have no automated versioning strategy tied to the build pipeline — version strings are either absent or manually maintained. This change introduces a consistent versioning scheme that distinguishes pre-release builds (`:develop` Docker tags) from release builds, and propagates those version strings into the running application.

## What Changes

- Define a versioning scheme: stable releases use semver tags (e.g. `1.2.3`); pre-release builds on the `develop` branch use a pre-release suffix (e.g. `1.2.3-pre.<sha>` or `0.0.0-develop.<sha>`)
- Wire version injection into the backend build so the binary knows its own version at runtime
- Wire version injection into the frontend build so the SvelteKit app knows its version at runtime
- Expose the version on the existing `/api/health` endpoint
- Docker image tags reflect the version: `:latest` / `:<semver>` for releases, `:develop` for pre-release builds
- CI/CD pipeline (or build scripts) increment and apply versions automatically

## Capabilities

### New Capabilities

- `release-versioning`: Version scheme, build-time injection for backend and frontend, Docker tag strategy, and CI automation for release vs pre-release builds

### Modified Capabilities

- `api-health`: Extend the health response to include the running application version

## Impact

- Backend: version string injected at compile time (e.g. via `cargo` build script or environment variable read at startup); surfaced on `/api/health`
- Frontend: version string injected at build time via SvelteKit environment variable
- Docker: image tagging convention updated in Compose and any CI pipeline definitions
- `project-infrastructure` spec may be affected if build/deploy conventions are documented there
