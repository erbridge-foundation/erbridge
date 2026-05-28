## MODIFIED Requirements

### Requirement: GET /api/health returns a flat status document

`GET /api/health` SHALL return HTTP 200 with `Content-Type: application/json; charset=utf-8` and a body of the form:

```json
{
  "status": "ok" | "degraded",
  "version": "<string>",
  "commit": "<string>",
  "components": [
    { "name": "<string>", "status": "ok" | "degraded" }
  ]
}
```

- `status` SHALL be the **overall** health status (see "overall status aggregation" below).
- `version` SHALL be the git-tag-derived application version baked in at compile time (see the `release-versioning` capability). Concretely it is the value of `CARGO_PKG_VERSION` as compiled, which `build.rs` overrides from the build-time `APP_VERSION` when present, falling back to the `Cargo.toml` value otherwise. (This supersedes the previous wording, which tied `version` to the `Cargo.toml` `CARGO_PKG_VERSION` directly; the manifest version is no longer the source of truth.)
- `commit` SHALL be the short git commit SHA captured at compile time, or the literal string `"unknown"` if the build environment did not provide one.
- `components` SHALL be a JSON array of component status objects. In this change the array contains exactly one element with `name = "db"`. Future changes MAY add more components without breaking the contract.

The endpoint SHALL NOT include any `{ "data": ... }` envelope. The response body is the flat object above.

The endpoint SHALL be publicly reachable (no auth) and MUST NOT consult the session store or any API key.

#### Scenario: Healthy backend returns 200 and status ok
- **WHEN** an unauthenticated client `GET /api/health` and Postgres is reachable
- **THEN** the response is HTTP 200 with body containing `"status": "ok"`, `"version": "<non-empty string>"`, `"commit": "<non-empty string>"`, and `"components"` containing `{ "name": "db", "status": "ok" }`

#### Scenario: Response is not enveloped
- **WHEN** `GET /api/health` returns a body
- **THEN** the body's top-level keys are exactly `status`, `version`, `commit`, `components` (no `data` key, no envelope wrapping)

#### Scenario: Public access
- **WHEN** a request to `GET /api/health` is made with no session cookie and no `Authorization` header
- **THEN** the response is HTTP 200 (not 401, not 403)

#### Scenario: Version reflects the git-derived APP_VERSION when built for release
- **WHEN** the backend is built with `APP_VERSION=1.2.3` (e.g. from a `v1.2.3` tag in CI)
- **THEN** `GET /api/health` returns `"version": "1.2.3"`

#### Scenario: Commit unknown when build did not capture one
- **WHEN** the backend was built without git metadata available (e.g. from a source tarball)
- **THEN** the response body's `commit` field is the literal string `"unknown"` and `status` is unaffected by this
