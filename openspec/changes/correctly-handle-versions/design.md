## Context

The application builds Docker images for the backend (Rust/axum) and frontend (SvelteKit/Node). Currently there is no automated mechanism to inject a meaningful version string at build time or to distinguish pre-release builds from stable releases. The `api-health` spec already defines `version` and `commit` fields on the health response, but their values depend on the build pipeline correctly supplying `CARGO_PKG_VERSION` and a git SHA.

## Goals / Non-Goals

**Goals:**
- Define a versioning scheme that covers both release and pre-release builds
- Inject version and commit SHA into the backend at compile time
- Inject version into the frontend at build time
- Tag Docker images consistently with the version (`:develop` for pre-release, `:<semver>` + `:latest` for release)
- Automate version determination in the build pipeline (CI or build scripts)

**Non-Goals:**
- Automatic semver bumping or changelog generation (version numbers are set by the developer/release process)
- Exposing frontend version via the API (backend version on `/api/health` is sufficient)
- Supporting multiple simultaneous release trains

## Decisions

<!-- Key technical decisions to be determined during implementation — e.g. how version is derived from git tags, whether `cargo-release` or a build script is used, whether the frontend version comes from `package.json` or an env var -->

## Risks / Trade-offs

<!-- To be identified during implementation -->

## Open Questions

- Should the version be driven by git tags (e.g. `git describe`) or by `Cargo.toml` / `package.json`?
- Who/what increments the version for a release — developer, CI, or a release tool?
- Is the pre-release suffix based on commit SHA, build number, or timestamp?
