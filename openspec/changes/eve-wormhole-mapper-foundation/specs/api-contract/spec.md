## ADDED Requirements

### Requirement: API routes are served under a major-version prefix

All `/api/*` routes SHALL be served under a major-version path prefix of the form `/api/v<N>/...`, where `<N>` is a positive integer. The current major version is `v1`; all routes introduced by this change live under `/api/v1/`.

- A new major version SHALL be introduced only for genuinely breaking changes (response/request shape changes that existing clients cannot tolerate, removed endpoints, semantics changes). Additive changes (new optional fields, new endpoints) SHALL NOT bump the version.
- When a new major version is introduced, the previous major version SHALL continue to be served for a deprecation window; the two versions MAY share handlers internally but SHALL be addressable independently in the URL.
- The success envelope, error envelope, and canonical error codes defined in this spec are considered frozen across major versions — a change to the envelope itself is a project-wide event, not a per-endpoint version bump.

#### Scenario: All API routes live under /api/v1/
- **WHEN** the backend exposes any `/api/*` route in this change
- **THEN** the route path begins with `/api/v1/`

#### Scenario: Unversioned /api/ path is not served
- **WHEN** a request targets `/api/keys` (no version segment)
- **THEN** the backend does not route the request to a v1 handler (the path either 404s or is explicitly unmapped)

### Requirement: JSON response envelope for /api/*

All successful `/api/*` responses with a body SHALL use a single JSON envelope of the form:

```json
{ "data": <payload>, "meta": <object?> }
```

- `data` SHALL contain the endpoint's payload. For single-resource endpoints it is an object; for collections it is an array.
- `meta` is OPTIONAL. When present it SHALL be a JSON object. It carries response-level information (pagination, request identifiers, etc.) that is not part of the resource itself.
- Responses with no body (HTTP 204) SHALL NOT include an envelope.
- The `Content-Type` of every JSON response SHALL be `application/json; charset=utf-8`.

This envelope applies to all routes under `/api/*`. The `/auth/*` routes are HTML redirects and cookie-setting endpoints; they are out of scope for this envelope.

#### Scenario: Single resource is wrapped in data
- **WHEN** a `/api/*` endpoint returns a single resource
- **THEN** the body is `{ "data": { ... } }` and the `Content-Type` is `application/json; charset=utf-8`

#### Scenario: Collection is wrapped in data as an array
- **WHEN** a `/api/*` endpoint returns a list
- **THEN** the body is `{ "data": [ ... ] }` and `data` is a JSON array (never a bare top-level array)

#### Scenario: Meta is omitted when empty
- **WHEN** a response has no meta information to convey
- **THEN** the `meta` key is omitted from the envelope (not present as `null` or `{}`)

#### Scenario: 204 responses have no body
- **WHEN** an endpoint completes successfully but has nothing to return (e.g. `DELETE`)
- **THEN** the response is HTTP 204 with an empty body and no envelope

### Requirement: JSON error envelope for /api/*

All `/api/*` error responses (HTTP 4xx and 5xx) with a body SHALL use a single JSON envelope of the form:

```json
{
  "error": {
    "code": "<machine_code>",
    "message": "<human-readable string>",
    "details": <object?>
  }
}
```

- `error.code` SHALL be a stable, lowercase, snake_case machine identifier (e.g. `invalid_api_key`, `validation_failed`, `not_found`). It is independent of the HTTP status code. Clients SHALL be able to branch on `error.code` rather than parsing `error.message`.
- `error.message` SHALL be a human-readable string. It MAY change between releases; clients SHALL NOT branch on its content.
- `error.details` is OPTIONAL. When present it SHALL be a JSON object carrying machine-readable specifics — typically field-level validation maps for `validation_failed`, or other code-specific extensions.
- The response `Content-Type` SHALL be `application/json; charset=utf-8` (not `application/problem+json`).
- Error envelopes SHALL NOT leak internal state (stack traces, SQL fragments, internal IDs of other accounts).

#### Scenario: 4xx error returns the error envelope
- **WHEN** any `/api/*` endpoint rejects a request with a 4xx status and a body
- **THEN** the body is `{ "error": { "code": "<…>", "message": "<…>" } }` with `Content-Type: application/json; charset=utf-8`

#### Scenario: Validation failure uses details for field errors
- **WHEN** a request body fails validation
- **THEN** the response is HTTP 400 with `error.code = "validation_failed"` and `error.details` is a JSON object mapping field names to per-field error messages

#### Scenario: error.code is stable, error.message is not
- **WHEN** two releases return the same logical error
- **THEN** `error.code` is identical across releases; `error.message` MAY differ

#### Scenario: Internal errors do not leak implementation details
- **WHEN** the backend returns HTTP 500
- **THEN** the response body is `{ "error": { "code": "internal_error", "message": "<generic>" } }` and SHALL NOT contain stack traces, panic payloads, or database error text

### Requirement: Canonical error codes for shared failure modes

The following `error.code` values are reserved and SHALL be used for the indicated failure modes wherever they occur under `/api/*`:

| code                | typical status | meaning                                                    |
| ------------------- | -------------- | ---------------------------------------------------------- |
| `unauthenticated`   | 401            | No valid credentials presented                             |
| `forbidden`         | 403            | Authenticated but not authorised for this resource/scope   |
| `not_found`         | 404            | Target resource does not exist or is not visible to caller |
| `conflict`          | 409            | Request conflicts with current state (e.g. duplicate name) |
| `validation_failed` | 400            | Request body or parameters failed validation               |
| `internal_error`    | 500            | Unhandled server-side failure                              |

Endpoints MAY introduce additional codes for endpoint-specific failures. Endpoint-specific codes SHALL be documented in the relevant capability spec.

#### Scenario: Missing or invalid credentials use unauthenticated
- **WHEN** a `/api/*` request has no valid session cookie and no valid bearer token
- **THEN** the response is HTTP 401 with `error.code = "unauthenticated"`

#### Scenario: Authorised-but-forbidden uses forbidden
- **WHEN** a `/api/*` request is authenticated but the caller is not permitted (e.g. server-scoped key on an account-scoped route)
- **THEN** the response is HTTP 403 with `error.code = "forbidden"`

#### Scenario: Missing or hidden resource uses not_found
- **WHEN** a `/api/*` request targets a resource that does not exist OR exists but is not visible to the caller
- **THEN** the response is HTTP 404 with `error.code = "not_found"` (existence is not disclosed)

#### Scenario: Conflicting state uses conflict
- **WHEN** a `/api/*` request would violate a uniqueness or state constraint (e.g. duplicate name)
- **THEN** the response is HTTP 409 with `error.code = "conflict"`

### Requirement: Timestamps are RFC 3339 UTC

Every timestamp in any `/api/*` request or response body SHALL be a string in RFC 3339 format with a `Z` suffix (UTC), e.g. `2026-05-17T14:32:10Z`. Fractional seconds are OPTIONAL; when present they SHALL use a `.` separator and MAY be up to microsecond precision (`2026-05-17T14:32:10.123456Z`). Timestamps SHALL NOT use non-UTC offsets in API payloads.

#### Scenario: Response timestamps are UTC RFC 3339
- **WHEN** any `/api/*` response includes a timestamp field
- **THEN** the value is a string matching `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$`

#### Scenario: Request timestamps are accepted as RFC 3339
- **WHEN** a `/api/*` request body contains a timestamp field
- **THEN** the backend accepts RFC 3339 strings with `Z` suffix and rejects values with non-UTC offsets or non-RFC-3339 formats with `error.code = "validation_failed"`

### Requirement: Machine-readable API description

The backend SHALL publish an OpenAPI 3.1 description of every `/api/*` route, covering for each route: HTTP method and path, request body schema (where applicable), response body schema for each status code, required authentication, and the canonical `error.code` values it may return.

- The description SHALL be derived from the running code (handler signatures and DTOs) via `utoipa` annotations, not maintained by hand alongside it.
- The description SHALL be available at `/api/openapi.json` and SHALL parse as valid OpenAPI 3.1.
- A human-browsable Swagger UI of the same document SHALL be available at `/api/docs`.
- The description SHALL be strictly faithful to the implementation: a backend test SHALL serialise representative responses from every documented route and validate them against the schema in the published OpenAPI document; documentation drift SHALL fail the build.
- The success envelope (`{ data, meta? }`) and error envelope (`{ error: { code, message, details? } }`) SHALL be expressed as reusable OpenAPI components and referenced by every relevant route, so envelope changes are made once and propagate.

Frontend type generation from the published OpenAPI document is **deferred to a future change**. Until that change lands, the frontend MAY hand-maintain TypeScript types for `/api/*` request and response shapes; this exception applies only to the foundation change and SHALL be removed once a generator is wired in.

#### Scenario: OpenAPI document is published by the backend
- **WHEN** the backend is running
- **THEN** `GET /api/openapi.json` returns HTTP 200 with `Content-Type: application/json` and a body that parses as valid OpenAPI 3.1

#### Scenario: Swagger UI is browsable
- **WHEN** a browser visits `/api/docs`
- **THEN** the response renders the Swagger UI bound to `/api/openapi.json`

#### Scenario: Document is derived from code
- **WHEN** a handler's request or response shape changes
- **THEN** the published OpenAPI document reflects the change without a separate hand-edit step

#### Scenario: Drift between document and implementation fails the build
- **WHEN** a handler returns a response shape that does not validate against its declared OpenAPI schema
- **THEN** the backend test suite fails; the build SHALL NOT pass with an out-of-date document

#### Scenario: Every documented /api/v1 route appears in the document
- **WHEN** the document is fetched
- **THEN** every route handler mounted under `/api/v1/` is present in the `paths` object with at least one documented response

#### Scenario: Envelope components are reused
- **WHEN** the document is fetched
- **THEN** the success and error envelopes are declared once under `components.schemas` and referenced (`$ref`) by every route response, not inlined per-route
