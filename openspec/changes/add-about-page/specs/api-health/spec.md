## ADDED Requirements

This capability defines the `/api/health` endpoint. It is intentionally exempt from the `api-contract` success envelope (see the `api-contract` MODIFIED requirement in this change) so that orchestration tooling can shallow-parse the response. The endpoint is public — it requires no session cookie and no API key. In this codebase auth is enforced per-handler by the `AuthenticatedAccount` extractor (`backend/src/handlers/middleware.rs`), not by a router-tree middleware split; a handler is public precisely by **not** naming that extractor in its signature. `/api/health` is therefore public by simply omitting the extractor, and is registered in `build_router` (`backend/src/lib.rs`) alongside `/api/openapi.json` and `/api/docs`.

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
- `version` SHALL be the backend `CARGO_PKG_VERSION` at compile time.
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

#### Scenario: Commit unknown when build did not capture one
- **WHEN** the backend was built without git metadata available (e.g. from a source tarball)
- **THEN** the response body's `commit` field is the literal string `"unknown"` and `status` is unaffected by this

### Requirement: Overall status aggregates component statuses

The top-level `status` field SHALL be derived from the `components` array:

- `status` SHALL be `"ok"` if and only if **every** element of `components` has `status = "ok"`.
- `status` SHALL be `"degraded"` if **any** element of `components` has `status = "degraded"`.

The handler SHALL compute this on every request from the live component statuses; it MUST NOT be stored or cached.

#### Scenario: All components ok yields overall ok
- **WHEN** every element of `components` has `status = "ok"`
- **THEN** the top-level `status` is `"ok"`

#### Scenario: Any component degraded yields overall degraded
- **WHEN** at least one element of `components` has `status = "degraded"`
- **THEN** the top-level `status` is `"degraded"`

### Requirement: db component reflects Postgres reachability

The `db` component SHALL report `status = "ok"` when the handler can successfully execute a trivial query (e.g. `SELECT 1`) against the application's Postgres pool, and `status = "degraded"` when that query returns an error.

The handler SHALL execute this check on every `GET /api/health` call. It SHALL NOT cache the result. The check SHALL use the same `PgPool` the application uses for `/auth/*` and `/api/v1/*` traffic.

#### Scenario: Postgres reachable
- **WHEN** the backend's Postgres pool can execute `SELECT 1` successfully
- **THEN** the `db` component reports `status = "ok"`

#### Scenario: Postgres unreachable
- **WHEN** the backend's Postgres pool cannot execute `SELECT 1` (connection refused, timeout, auth failure, etc.)
- **THEN** the `db` component reports `status = "degraded"` and the response is still HTTP 200 (the endpoint does not 5xx; "degraded" is the success signal)

### Requirement: Every registered `/api/v1` route declares authentication

Because auth is opt-in per handler (a handler is public by omitting the `AuthenticatedAccount` extractor), an accidentally-omitted extractor would silently expose a versioned business route. Introducing the deliberately-public `/api/health` makes that failure mode newly easy to overlook. To keep the versioned surface fail-closed, every route registered under `/api/v1` SHALL declare an authentication requirement.

Concretely: for every `(path, method)` in `backend::registered_api_v1_routes()`, the OpenAPI document's operation at that path/method SHALL carry a non-empty `security` requirement. A test SHALL enforce this. `/api/health` is **not** a member of `registered_api_v1_routes()` and is therefore out of scope for this requirement — the guard polices the versioned business surface, not the observability carve-outs (`/api/health`, `/api/openapi.json`, `/api/docs`).

#### Scenario: A registered v1 route without declared auth fails the guard
- **WHEN** a route is registered in `registered_api_v1_routes()` and its OpenAPI operation declares no `security` requirement
- **THEN** the authentication-coverage test fails, naming the offending route

#### Scenario: The public health route is exempt
- **WHEN** the authentication-coverage test runs
- **THEN** `/api/health` is not asserted against (it is not in `registered_api_v1_routes()`), and its lack of a `security` requirement does not fail the test
