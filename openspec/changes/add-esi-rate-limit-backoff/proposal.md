## Why

The backend has no rate limiting in either direction. Outbound, all ESI calls share a single deployment IP, and ESI enforces an error-rate budget **per IP** (~100 errors per rolling 60s window): exhaust it and CCP returns HTTP 420 and temporarily blocks *every* request from our IP — token refresh, search, the lot. A bad token-sweep or a tight search loop can take the whole instance offline with CCP. Inbound, our own `/api/*` endpoints (notably the ESI-backed search and auth routes) are open to unbounded request volume from any caller, with no per-IP throttle to blunt abuse or accidental hammering.

## What Changes

- **Outbound (the priority):** Add an ESI-rate-limit-aware backoff layer to the shared `reqwest_middleware` HTTP client used by every ESI call. ESI runs **two coexisting limiters** (per CCP's official rate-limiting docs): a newer **token bucket** keyed per `(rate-limit-group, userID)` that meters *all* responses (2xx=2, 3xx=1, 4xx=5, 5xx=0 tokens) and returns **429 + `Retry-After`** on exhaustion (`X-Ratelimit-*` headers); and the legacy **per-IP error budget** (~100 non-2xx/3xx per 60s) that returns **420** on exhaustion (`X-Esi-Error-Limit-*` headers). The layer reads whichever header set a response carries and backs off accordingly — per-bucket waits honouring `Retry-After` for 429, and a process-wide per-IP wait for 420. It surfaces both as errors to callers (token sweep, search, public-info) rather than masking them.
- **Inbound (`/api/*`):** Add a per-client-IP request-rate limiter as a tower layer in `build_router`, returning HTTP 429 with the project's standard JSON error envelope and a new `rate_limited` canonical error code, plus a `Retry-After` header.
- **Inbound (`/auth/*`):** Add a separate, independently-tuned per-IP limiter covering the auth routes — chiefly `/auth/callback`, the most expensive unauthenticated endpoint (SSO token exchange + several ESI calls + session write, reachable pre-session). Because the api-contract exempts `/auth/*` from the JSON envelope, a throttled auth request **redirects to a dedicated "too busy" page** rather than returning the `rate_limited` envelope. This also stops abuse of `/auth/callback` from draining the outbound ESI error budget.
- Add a new canonical error code `rate_limited` to the api-contract.
- Add dependencies: `tower_governor` (inbound) and a small amount of shared state for the outbound gates (no new outbound crate strictly required — implemented as a custom `reqwest_middleware::Middleware`).
- **Not doing:** a *fixed* self-chosen requests/sec cap on ESI. We react to the budgets ESI itself reports rather than inventing our own ceiling. Design records this.

## Capabilities

### New Capabilities
- `esi-rate-limiting`: How the backend stays within ESI's two limiters — the per-`(group, userID)` token bucket (`X-Ratelimit-*`, 429/`Retry-After`) and the legacy per-IP error budget (`X-Esi-Error-Limit-*`, 420) — including the safety thresholds, per-bucket vs process-wide waits, and that 420/429 surface as errors to callers.
- `api-rate-limiting`: Inbound per-IP request throttling for `/api/*` routes — the limit policy, the 429 response shape, and the `Retry-After` header.
- `auth-rate-limiting`: Inbound per-IP throttling for `/auth/*` (chiefly `/auth/callback`) — a separate limiter that redirects to a "too busy" page on reject (not the JSON envelope) and shields the ESI budget from pre-session abuse.

### Modified Capabilities
- `api-contract`: Adds `rate_limited` to the set of canonical error codes and specifies that a throttled request returns the standard error envelope with HTTP 429.

## Impact

- **Code:** `backend/src/main.rs` (client middleware chain at the `ClientBuilder` seam), a new outbound middleware module under `backend/src/esi/`, `backend/src/lib.rs` (two `build_router` layers — one for `/api/*`, one for `/auth/*` — plus `registered_*_routes`-adjacent test surface), the response/error module for the new `rate_limited` code, `backend/src/openapi.rs` for the documented 429, and a dedicated "too busy" redirect target (frontend route is a follow-up).
- **Dependencies:** add `tower_governor`; outbound gate uses existing `reqwest-middleware` + `tokio` sync primitives.
- **Tests:** unit tests for the error-limit gate (threshold, reset, 420), an integration test asserting inbound 429 + envelope + `Retry-After`, and HURL coverage for a throttled response.
- **Behaviour:** under sustained load or error conditions the backend will now *deliberately pause* outbound calls rather than barrel into a 420 block or 429 throttle; callers (token sweep, search) must tolerate the added latency. The happy path is largely unchanged, but note the token bucket meters successful calls too, so very high 2xx volume can now also trigger a back-off.
