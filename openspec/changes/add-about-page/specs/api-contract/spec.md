## MODIFIED Requirements

### Requirement: JSON response envelope for /api/*

All successful `/api/*` responses with a body SHALL use a single JSON envelope of the form:

```json
{ "data": <payload>, "meta": <object?> }
```

- `data` SHALL contain the endpoint's payload. For single-resource endpoints it is an object; for collections it is an array.
- `meta` is OPTIONAL. When present it SHALL be a JSON object. It carries response-level information (pagination, request identifiers, etc.) that is not part of the resource itself.
- Responses with no body (HTTP 204) SHALL NOT include an envelope.
- The `Content-Type` of every JSON response SHALL be `application/json; charset=utf-8`.

This envelope applies to all routes under `/api/*` with two documented exceptions:

1. The `/auth/*` routes are HTML redirects and cookie-setting endpoints; they are out of scope for this envelope.
2. The `/api/health` route returns a flat status document instead of the envelope, for compatibility with orchestration tooling (container healthchecks, Traefik / k8s liveness probes, external uptime monitors) that shallow-parses the response. The flat shape is defined by the `api-health` capability.

Adding a new exception requires amending this requirement in a dedicated change; ad-hoc exceptions are not permitted.

#### Scenario: Single resource is wrapped in data
- **WHEN** a `/api/*` endpoint other than `/api/health` returns a single resource
- **THEN** the body is `{ "data": { ... } }` and the `Content-Type` is `application/json; charset=utf-8`

#### Scenario: Collection is wrapped in data as an array
- **WHEN** a `/api/*` endpoint other than `/api/health` returns a list
- **THEN** the body is `{ "data": [ ... ] }` and `data` is a JSON array (never a bare top-level array)

#### Scenario: Meta is omitted when empty
- **WHEN** a response has no meta information to convey
- **THEN** the `meta` key is omitted from the envelope (not present as `null` or `{}`)

#### Scenario: 204 responses have no body
- **WHEN** an endpoint completes successfully but has nothing to return (e.g. `DELETE`)
- **THEN** the response is HTTP 204 with an empty body and no envelope

#### Scenario: /api/health is exempt from the envelope
- **WHEN** a request is made to `GET /api/health`
- **THEN** the response body is a flat JSON object per the `api-health` capability (not wrapped in `{ "data": ... }`), and this is the only `/api/*` exception besides `/auth/*`
