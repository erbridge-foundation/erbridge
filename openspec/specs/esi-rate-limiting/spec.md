## Purpose

Outbound rate-limit discipline for the backend's calls to EVE's ESI API. ESI enforces two coexisting, mutually-exclusive limiters — a per-`(group, userID)` token bucket (429) and a legacy per-IP error budget (420) — and this capability defines how the backend observes both, backs off per bucket as the token bucket nears exhaustion, gates all callers through a process-wide error budget, and treats an HTTP 420 as a hard stop-and-wait.

## Requirements

### Requirement: Both ESI rate-limit systems are observed

ESI enforces two coexisting, mutually-exclusive limiters and the backend SHALL respect both:

1. The **token-bucket rate limiter** (newer, on rate-limited routes): a floating-window bucket keyed per `(rate-limit-group, userID)`. Each response consumes tokens (2xx=2, 3xx=1, 4xx=5 excluding 429, 5xx=0). Exhaustion returns HTTP 429 with `Retry-After`. Signalled by the `X-Ratelimit-Group`, `X-Ratelimit-Limit`, `X-Ratelimit-Remaining`, and `X-Ratelimit-Used` headers.
2. The **legacy error limiter** (on non-rate-limited routes): at most ~100 non-2xx/3xx responses per rolling 60s window, **per source IP**. Exhaustion returns HTTP 420 on all ESI routes. Signalled by the `X-Esi-Error-Limit-Remain` and `X-Esi-Error-Limit-Reset` headers.

The backend SHALL read whichever set of headers a response carries and SHALL NOT assume a response carries both — they are documented as mutually exclusive per route.

#### Scenario: Token-bucket headers are recorded when present
- **WHEN** an ESI response carries `X-Ratelimit-*` headers
- **THEN** the backend records the remaining tokens and limit window for that `(group, userID)` bucket

#### Scenario: Legacy error-limit headers are recorded when present
- **WHEN** an ESI response carries `X-Esi-Error-Limit-Remain` / `X-Esi-Error-Limit-Reset`
- **THEN** the backend records the remaining error count and reset window for the process-wide (per-IP) error budget

#### Scenario: Headers are read regardless of status code
- **WHEN** an ESI response is a success (2xx) carrying either header set
- **THEN** the backend updates the corresponding budget, because both limiters report on success responses too

#### Scenario: Missing headers do not crash the client
- **WHEN** an ESI response omits both header sets
- **THEN** the backend leaves recorded budgets unchanged and the request proceeds normally

### Requirement: Outbound requests back off as the token bucket nears exhaustion

For the token-bucket limiter, the backend SHALL track remaining tokens per `(rate-limit-group, userID)` bucket as reported by `X-Ratelimit-Remaining` / `X-Ratelimit-Limit`. When a bucket's remaining tokens fall at or below a configured safety threshold, the backend SHALL delay new requests that would draw on that bucket until the floating window releases enough tokens. The `userID` SHALL be derived the way ESI derives it: `<applicationID>:<characterID>` for authenticated routes, and the source identity (IP, optionally with applicationID) for unauthenticated routes — so back-off is scoped per character/app rather than globally throttling unrelated callers.

#### Scenario: Below threshold, requests on that bucket wait
- **WHEN** a bucket's `X-Ratelimit-Remaining` is at or below the safety threshold
- **AND** a new request that would draw on that same `(group, userID)` bucket is initiated
- **THEN** the request is held until the window releases tokens, then proceeds

#### Scenario: Above threshold, requests are not delayed
- **WHEN** a bucket's remaining tokens are above the safety threshold
- **THEN** new requests drawing on that bucket proceed immediately with no added latency

#### Scenario: Back-off is scoped per bucket, not global
- **WHEN** one `(group, userID)` bucket is near exhaustion
- **THEN** requests drawing on a different bucket (different group or different character/app) are not delayed by it

#### Scenario: 429 is a hard wait honouring Retry-After
- **WHEN** an ESI request returns HTTP 429 with a `Retry-After` header
- **THEN** subsequent requests on that bucket are held until `Retry-After` elapses
- **AND** the originating call receives an error/unavailable outcome (not a success)

### Requirement: Legacy error budget is observed via a process-wide per-IP gate

For the legacy error limiter, the backend SHALL maintain a single process-wide gate shared across all ESI callers (token sweep, search, public-info, token refresh), reflecting CCP's per-IP error budget. When the recorded `X-Esi-Error-Limit-Remain` falls at or below a configured safety threshold, the gate SHALL delay new ESI requests until the current reset window has elapsed.

#### Scenario: Below threshold, new requests wait for the reset window
- **WHEN** the recorded `X-Esi-Error-Limit-Remain` is at or below the safety threshold
- **AND** a new ESI request is initiated before the reset window elapses
- **THEN** the request is held until the reset window has elapsed, then proceeds

#### Scenario: Above threshold, requests are not delayed
- **WHEN** the recorded error budget is above the safety threshold
- **THEN** new ESI requests proceed immediately with no added latency

#### Scenario: Error gate is shared across all callers
- **WHEN** one ESI caller has driven the error budget below the threshold
- **THEN** ESI requests from every other caller are subject to the same wait, because the legacy error budget is a single per-IP budget

### Requirement: HTTP 420 from ESI is a hard stop-and-wait

The backend SHALL treat an ESI HTTP 420 response as an exhausted legacy error budget. On 420, the backend SHALL stop issuing new ESI requests until the reset window indicated by `X-Esi-Error-Limit-Reset` has elapsed, and SHALL surface the 420 to the calling code as a retryable/unavailable outcome rather than masking it as success. A 420 affects all ESI routes, so the stop applies to the process-wide gate, not a single bucket.

#### Scenario: 420 blocks all subsequent requests for the reset window
- **WHEN** an ESI request returns HTTP 420 with a reset window
- **THEN** subsequent ESI requests across all callers are held until that window elapses
- **AND** the originating call receives an error outcome (not a success)
