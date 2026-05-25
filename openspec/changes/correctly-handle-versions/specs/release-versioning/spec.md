## ADDED Requirements

### Requirement: Version scheme distinguishes release from pre-release
The project SHALL use a versioning scheme where stable releases are identified by a semver string (e.g. `1.2.3`) and pre-release builds are identified by a pre-release suffix (e.g. `1.2.3-pre.<short-sha>`). The `develop` branch SHALL always produce pre-release-versioned builds.

#### Scenario: Release build has clean semver
- **WHEN** a build is triggered from a release tag
- **THEN** the version string SHALL be a clean semver (e.g. `1.2.3`) with no pre-release suffix

#### Scenario: Pre-release build has suffix
- **WHEN** a build is triggered from the `develop` branch or any non-release ref
- **THEN** the version string SHALL include a pre-release suffix (e.g. `1.2.3-pre.<sha>`)

### Requirement: Backend version injected at compile time
The backend binary SHALL have its version string baked in at compile time so that `GET /api/health` returns a meaningful `version` value without requiring a runtime environment variable.

#### Scenario: Version available in running backend
- **WHEN** the backend binary is started
- **THEN** `GET /api/health` SHALL return a non-empty `version` field matching the version at which the binary was built

### Requirement: Frontend version injected at build time
The frontend SvelteKit application SHALL have its version string injected as a build-time environment variable so it can be displayed in the UI if needed.

#### Scenario: Version available in built frontend
- **WHEN** the frontend is built
- **THEN** the version string SHALL be accessible as a build-time constant within the application

### Requirement: Docker images tagged with version
Docker images for the backend and frontend SHALL be tagged consistently: release builds receive both `:<semver>` and `:latest` tags; pre-release builds from the `develop` branch receive a `:develop` tag.

#### Scenario: Release image tagged correctly
- **WHEN** a release build produces Docker images
- **THEN** images SHALL be tagged with `:<semver>` and `:latest`

#### Scenario: Pre-release image tagged correctly
- **WHEN** a pre-release build produces Docker images from the `develop` branch
- **THEN** images SHALL be tagged with `:develop`
