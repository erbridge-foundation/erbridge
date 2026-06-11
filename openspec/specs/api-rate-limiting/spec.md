## Purpose

Inbound request-rate limiting for the backend's `/api/*` routes: a per-client-IP sustained rate with a burst allowance, applied as a router-level layer, returning HTTP 429 with the project's standard error envelope and a `Retry-After` header when exceeded.

## Requirements

### Requirement: Inbound requests are rate limited per client IP

The backend SHALL apply a per-client-IP request-rate limit to `/api/*` routes. The limit SHALL be expressed as a sustained request rate with a burst allowance, applied as a router-level layer so it covers every `/api/*` route uniformly. Requests in excess of the limit SHALL be rejected without reaching the handler.

#### Scenario: Requests within the limit are served normally
- **WHEN** a client sends `/api/*` requests within the configured rate and burst
- **THEN** every request is routed to its handler as normal

#### Scenario: Requests exceeding the limit are rejected
- **WHEN** a client exceeds the configured rate and burst for `/api/*`
- **THEN** the excess requests are rejected before reaching the handler

#### Scenario: Limit is tracked per client IP
- **WHEN** two clients with different source IPs send requests
- **THEN** each client's allowance is tracked independently; one client exhausting its budget does not throttle the other

### Requirement: Throttled responses use 429 with the standard error envelope and Retry-After

A throttled `/api/*` request SHALL return HTTP 429 with the project's standard JSON error envelope, `error.code = "rate_limited"`, and a `Retry-After` header indicating when the caller may retry. The error envelope SHALL NOT leak internal limiter state.

#### Scenario: Throttled request returns the rate_limited envelope
- **WHEN** an `/api/*` request is rejected by the rate limiter
- **THEN** the response is HTTP 429 with `error.code = "rate_limited"` in the standard error envelope

#### Scenario: Throttled response includes Retry-After
- **WHEN** an `/api/*` request is throttled
- **THEN** the response carries a `Retry-After` header with the seconds until the caller may retry
