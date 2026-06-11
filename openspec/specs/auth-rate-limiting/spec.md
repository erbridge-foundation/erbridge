## Purpose

Inbound request-rate limiting for the backend's `/auth/*` routes — chiefly `/auth/callback`, the most expensive unauthenticated endpoint — applied as a per-client-IP limiter separate from the `/api/*` one. Because `/auth/*` are browser-redirect endpoints exempt from the JSON error envelope, throttled requests redirect to a dedicated "too busy" page rather than returning the `rate_limited` envelope.

## Requirements

### Requirement: Authentication routes are rate limited per client IP

The backend SHALL apply a per-client-IP request-rate limit to the `/auth/*` routes, separate from the `/api/*` limiter and tuned independently. The limit SHALL at minimum cover `/auth/callback`, which is the most expensive unauthenticated endpoint (it performs an SSO token exchange plus several outbound ESI calls and a session write per hit) and is reachable before any session exists. Requests in excess of the limit SHALL be rejected before reaching the handler.

#### Scenario: Callback requests within the limit are served normally
- **WHEN** a client sends `/auth/callback` requests within the configured rate and burst
- **THEN** each request proceeds to the handler and the normal SSO flow runs

#### Scenario: Excess auth requests are rejected before the handler
- **WHEN** a client exceeds the configured rate and burst for `/auth/*`
- **THEN** the excess requests are rejected before the handler runs, so no SSO token exchange or outbound ESI call is made for them

#### Scenario: Auth limit is tracked per client IP
- **WHEN** two clients with different source IPs send `/auth/*` requests
- **THEN** each client's allowance is tracked independently

### Requirement: Throttled auth requests redirect rather than return the JSON envelope

A throttled `/auth/*` request SHALL NOT use the `/api/*` JSON error envelope (the api-contract exempts `/auth/*` as browser-redirect endpoints). Instead the backend SHALL respond with an HTTP redirect to a dedicated "too busy" page so the browser flow stays consistent. The response SHALL NOT return the `rate_limited` JSON envelope used by `/api/*`.

#### Scenario: Throttled auth request redirects to the too-busy page
- **WHEN** an `/auth/*` request is rejected by the auth rate limiter
- **THEN** the response is an HTTP redirect to the dedicated too-busy page, not a JSON `rate_limited` envelope

#### Scenario: Auth throttling does not consume the ESI budget
- **WHEN** `/auth/callback` requests are being throttled at the inbound limiter
- **THEN** the rejected requests make no outbound ESI or SSO calls, so the throttle protects the ESI error budget rather than spending it
