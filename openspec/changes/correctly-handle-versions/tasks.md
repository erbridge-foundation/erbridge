## 1. Version scheme and tooling

- [ ] 1.1 Decide version source of truth (git tags vs `Cargo.toml`/`package.json`) and document in design.md
- [ ] 1.2 Implement version determination script/CI step that outputs semver for release tags and pre-release suffix for `develop` builds

## 2. Backend version injection

- [ ] 2.1 Add a `build.rs` (or equivalent) that captures the version string and git SHA at compile time
- [ ] 2.2 Verify `GET /api/health` returns the correct `version` and `commit` values for both release and pre-release builds

## 3. Frontend version injection

- [ ] 3.1 Pass the version string as a build-time environment variable during the SvelteKit build
- [ ] 3.2 Expose the version constant in the frontend codebase for potential UI use

## 4. Docker tagging

- [ ] 4.1 Update Docker build/push steps to tag release images with `:<semver>` and `:latest`
- [ ] 4.2 Update Docker build/push steps to tag pre-release (`develop`) images with `:develop`
- [ ] 4.3 Verify `docker-compose.dev.yml` continues to work correctly after tagging changes
