## MODIFIED Requirements

### Requirement: Canonical error codes for shared failure modes

The following `error.code` values are reserved and SHALL be used for the indicated failure modes wherever they occur under `/api/*`:

| code                | typical status | meaning                                                    |
| ------------------- | -------------- | ---------------------------------------------------------- |
| `unauthenticated`   | 401            | No valid credentials presented                             |
| `forbidden`         | 403            | Authenticated but not authorised for this resource/scope   |
| `not_found`         | 404            | Target resource does not exist or is not visible to caller |
| `conflict`          | 409            | Request conflicts with current state (e.g. duplicate name) |
| `validation_failed` | 400            | Request body or parameters failed validation               |
| `rate_limited`      | 429            | Caller exceeded the inbound request-rate limit             |
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

#### Scenario: Rate-limited request uses rate_limited
- **WHEN** a `/api/*` request is rejected by the inbound rate limiter
- **THEN** the response is HTTP 429 with `error.code = "rate_limited"` in the standard error envelope and a `Retry-After` header
