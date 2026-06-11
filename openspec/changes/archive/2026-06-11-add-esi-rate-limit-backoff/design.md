## Context

The backend has no rate limiting in either direction.

Outbound, every ESI call goes through a single shared `reqwest_middleware::ClientWithMiddleware` built once in `main.rs` (the `ClientBuilder::new(base_client).with(TracingMiddleware::default()).build()` chain) and stored on `AppState.http_client`. Callers — `esi::token`, `esi::search`, `esi::public_info`, and the `token_sweep` background task — all borrow that one client.

ESI enforces **two coexisting, mutually-exclusive limiters** (verified against CCP's official rate-limiting docs, June 2026 — `https://developers.eveonline.com/docs/services/esi/rate-limiting/`):

1. **Token-bucket limiter** (newer, on rate-limited routes). A floating-window bucket keyed per `(rate-limit-group, userID)`, where `userID` is `<applicationID>:<characterID>` on authenticated routes and the source IP (optionally `:applicationID`) on unauthenticated ones. Each response *spends* tokens — **2xx costs 2, 3xx 1, 4xx 5 (excluding 429), 5xx 0** — so even successful traffic is metered. Exhaustion returns **HTTP 429 + `Retry-After`**. Signalled by `X-Ratelimit-Group` / `-Limit` / `-Remaining` / `-Used`.
2. **Legacy error limiter** (on non-rate-limited routes). ~100 non-2xx/3xx responses per rolling 60s window, **per source IP**. Exhaustion returns **HTTP 420** on all ESI routes. Signalled by `X-Esi-Error-Limit-Remain` / `-Reset`. The two header sets are documented as mutually exclusive per route.

So the outbound design must handle **both**: a per-`(group, userID)` token-bucket back-off (the new system meters even 2xx, so we cannot assume the happy path is free), and a process-wide per-IP error gate for the legacy 420. Both live in the same middleware on the shared client.

Inbound, `build_router` applies only `refresh_session_cookie` and `TraceLayer`. There is no per-IP throttle on `/api/*`. The api-contract already defines a frozen error envelope and a canonical-error-code table; a new `rate_limited` code slots into that table.

Inbound, `build_router` applies only `refresh_session_cookie` and `TraceLayer`. There is no per-IP throttle on `/api/*`. The api-contract already defines a frozen error envelope and a canonical-error-code table; a new `rate_limited` code slots into that table.

## Goals / Non-Goals

**Goals:**
- Keep the backend within *both* ESI limiters so we never get 429-throttled or 420-blocked under normal operation.
- Back off reactively from ESI's own headers: per-`(group, userID)` token buckets (`X-Ratelimit-*`) and the per-IP error budget (`X-Esi-Error-Limit-*`).
- Apply that back-off at the shared client seam so it covers every ESI caller without per-call-site changes.
- Add a modest per-IP inbound throttle on `/api/*` that returns a spec-compliant 429.

**Non-Goals:**
- A *fixed* configured requests/sec cap on ESI. We do not invent our own rate ceiling — we react to the limits ESI reports. (This corrects an earlier draft that assumed 2xx traffic was uncounted; the token bucket meters 2xx, so we honour `X-Ratelimit-Remaining` rather than asserting a static cap.)
- Distributed / multi-instance coordination of the budgets. Today the backend is a single process on a single IP; a process-wide error gate is correct and the token-bucket state is per-process. Cross-instance coordination is deferred, tied to the scale-out decision.
- Re-keying around CCP's `userID` derivation perfectly. We mirror their `(group, userID)` keying as reported by the headers; we do not attempt to predict bucket assignment ahead of the first response on a route.

## Decisions

**Outbound: one custom `reqwest_middleware::Middleware` handling both limiters, not a new crate.**
Implement an `EsiRateLimitMiddleware` in `backend/src/esi/` (per rust-rest-api layout) holding two pieces of shared state behind `Arc`s:
- a **per-IP error gate**: a single `(remain, reset_at)` cell for the legacy `X-Esi-Error-Limit-*` budget / 420;
- a **token-bucket map**: `(group, userID) -> (remaining, window_until)` from `X-Ratelimit-*`, keyed exactly as ESI reports it.

In `handle`, before delegating it checks **both**: if the error gate is tripped (`remain <= threshold` and `now < reset_at`) it waits on the process-wide gate; and if the request's target bucket is known and near-exhausted it waits on that bucket. After the inner call it parses whichever header set the response carried, detects status 420 (→ error gate hard stop) and 429 (→ bucket hard wait honouring `Retry-After`), updates the relevant state, and surfaces 420/429 to the caller as an error. Added to the chain in `main.rs` after `TracingMiddleware`.
- *Why one middleware over two:* both react to headers on the same responses through the same shared client; splitting them duplicates the parse/await machinery.
- *Why over a generic limiter crate (e.g. `governor` on the client side):* generic limiters enforce a *fixed* rate we choose; here we must react to externally-reported budgets and bucket keys, which only a custom middleware can read.
- *Why the middleware seam over wrapping each `esi::*` fn:* there are four+ call sites and a background task; the middleware covers them all and any future caller for free, and keeps the per-IP gate genuinely process-wide.

**Bucket keying caveat.** We only learn a route's `(group, userID)` from its *first response's* headers, so the very first call on a cold bucket cannot be pre-gated — acceptable, since back-off matters under sustained load, not first contact. We do not try to pre-compute CCP's `userID` (`applicationID:characterID`) ourselves; we trust the returned `X-Ratelimit-Group` and our own knowledge of which token issued the call only as a fallback key.

**Threshold + reset semantics.** Each gate trips at a conservative remaining value (config-defaulted) rather than waiting for 0, leaving headroom for in-flight requests whose responses haven't yet updated the counter. On 420 treat the error budget as exhausted and honour `X-Esi-Error-Limit-Reset` as a hard process-wide wait; on 429 honour `Retry-After` as a hard wait on that bucket. Surface both to the caller as errors (must not look like success). This matches the esi-rate-limiting spec.

**Inbound: `tower_governor`.** Add `tower_governor` as a `GovernorLayer` in `build_router`, keyed by client IP (peer IP via `SmartIpKeyExtractor`, given the Traefik front; revisit `X-Forwarded-For` trust as part of implementation). Configure a sustained rate + burst. On rejection it must yield the project envelope, not governor's default body — wrap or configure the error response to emit `{ error: { code: "rate_limited", message } }` at HTTP 429 with `Retry-After`.
- *Why governor over a hand-rolled limiter:* it's the standard tower-ecosystem GCRA limiter, per-key, low overhead, already integrates as a layer.

**Auth: a separate `/auth/*` limiter that redirects on reject.**
`/auth/callback` (`handlers::auth::callback`) is the most expensive unauthenticated endpoint — per hit it does an SSO token exchange and fetches character/corp/alliance info over ESI, then writes a session. It is reachable pre-session, so it's the natural pre-auth abuse target, and abusing it with junk `code`/`state` burns the *outbound* ESI error budget via failed exchanges. Its outbound calls already route through the shared `http_client`, so the outbound middleware covers them; what's missing is an *inbound* throttle.

Add a second `GovernorLayer` scoped to `/auth/*` (at least `/auth/callback`; `login`/`add_character` are cheap redirect builders), tuned tighter and independently of the `/api/*` limiter. On reject it MUST NOT emit the `rate_limited` JSON envelope — the api-contract explicitly exempts `/auth/*` as browser-redirect endpoints, and a JSON body would break the browser flow. Instead it redirects (302) to a dedicated "too busy" page (distinct from `/blocked`, which carries a different meaning). The frontend route for that page is a follow-up; the backend redirect target is what this change defines.
- *Why a separate layer, not reuse the `/api/*` one:* different reject shape (redirect vs JSON envelope), different (tighter) limits, and different route prefix. Cleaner as two layers than one with branching.

**`rate_limited` error code.** Added to the canonical table in api-contract and to the backend's error/response module so the `/api/*` 429 path reuses the existing envelope machinery, and documented in `openapi.rs` for the affected routes (or globally, matching how shared codes are already published). The `/auth/*` redirect path does NOT use this code.

## Risks / Trade-offs

- **Threshold too aggressive → unnecessary stalls.** A high threshold pauses ESI traffic earlier than strictly required. → Separate config defaults for the error gate and the token bucket; log when either trips; tune from real `X-Esi-Error-Limit-Remain` / `X-Ratelimit-Remaining` telemetry.
- **Two-limiter state adds complexity / drift risk.** Tracking both an error gate and a per-bucket map is more moving parts than a single counter. → Keep them as two small, independently-tested units in one middleware; never assume a response carries both header sets (they are mutually exclusive); fall back to "leave unchanged" when neither is present.
- **Cold bucket cannot be pre-gated.** The first call on a route has no known `(group, userID)` budget yet. → Accepted; back-off matters under sustained load. The error gate still covers the first call's downside (a 420 path).
- **In-flight requests race the counters.** Concurrent ESI calls may all read a stale "above threshold" before any response lands. → Threshold headroom absorbs this; the gates need not be perfectly precise, only keep us clear of 420/429. Accept mild over/undershoot rather than serialising all ESI calls.
- **Lock contention could bottleneck ESI throughput.** → Keep locked sections tiny (read/update a few numbers); do every `sleep` outside the lock; the token-bucket map can be a sharded/concurrent map. Happy path stays lock-light.
- **Inbound limiter mis-keyed behind Traefik → everyone shares one bucket or trivially bypassable.** → Decide the IP-extraction strategy explicitly against the Traefik forwarded-headers config during implementation; cover with the integration test.
- **420/429 surfaced as error changes caller expectations.** Token sweep and search must tolerate a new unavailable/retryable outcome. → Align with the existing esi-search "unavailable outcome" pattern already in the codebase; verify sweep handles it without crashing.
- **Single-process assumption.** A second backend instance would have an independent error gate and its own token-bucket state; together they could exceed the per-IP error budget (the token bucket is keyed per character/app so is less affected). → Documented Non-Goal; revisit alongside the SSE-bus / scale-out decision.

## Migration Plan

Additive and behind no migration. Deploy: ship the outbound middleware first (pure safety improvement, no API change), then the inbound layer + `rate_limited` code. Rollback: remove the layer / middleware from the two builder seams; no schema or data changes to unwind. Both default thresholds/rates should ship conservative and be env-tunable.

## Open Questions

- Exact inbound rate + burst values for `/api/*` and (separately, tighter) for `/auth/*`, and whether admin routes warrant a distinct policy from account routes.
- The "too busy" redirect target path and whether the frontend route ships in this change or as an immediate follow-up; whether `login`/`add_character` are included in the auth limiter or only `/auth/callback`.
- Outbound safety thresholds: separate defaults for the legacy error gate (of ~100) and the token bucket (fraction of the reported `X-Ratelimit-Limit`), and whether reset waits should be the full window or a fraction with jitter to avoid a thundering-herd at window reset.
- Whether to honour CCP's per-route cache `Expires` header as well (re-requesting before expiry wastes token-bucket budget) — likely a follow-up, not this change.
- IP-extraction trust model behind Traefik (which forwarded header, and is it spoofable from outside the trusted hop).
