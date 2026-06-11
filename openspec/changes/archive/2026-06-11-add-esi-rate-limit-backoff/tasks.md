## 1. Dependencies

- [x] 1.1 Add `tower_governor` to `backend/Cargo.toml`
- [x] 1.2 Confirm `reqwest-middleware` + `tokio` (sync, time) features cover the outbound gate; no new outbound crate

## 2. Outbound: ESI dual-limiter backoff middleware

- [x] 2.1 Implement against the change's `esi-rate-limiting` spec (the capability lands in `openspec/specs/` only at archive time — no action now)
- [x] 2.2 Create `backend/src/esi/rate_limit.rs` (per rust-rest-api layout): an `EsiRateLimitMiddleware` implementing `reqwest_middleware::Middleware`, holding (a) a process-wide error gate `(remain, reset_at)` and (b) a `(group, userID) -> (remaining, window_until)` token-bucket map, both behind `Arc`s
- [x] 2.3 Before delegating in `handle`: wait on the error gate if `remain <= error_threshold` and `now < reset_at`; and wait on the target bucket if known and `remaining <= bucket_threshold` until its window releases. Sleep outside the lock; if the bucket is unknown (cold), proceed
- [x] 2.4 After the inner call: parse whichever header set is present — `X-Ratelimit-Group/-Limit/-Remaining/-Used` for the bucket, or `X-Esi-Error-Limit-Remain/-Reset` for the error gate (tolerate absence of both → leave state unchanged); update the relevant state
- [x] 2.5 Detect HTTP 420 → exhaust the error gate, honour `X-Esi-Error-Limit-Reset` as a process-wide hard wait. Detect HTTP 429 → hard wait on that bucket honouring `Retry-After`. Surface both to the caller as an error/unavailable outcome (not success)
- [x] 2.6 Make both thresholds + reset/jitter behaviour config-driven via `Config` (conservative defaults — error gate e.g. remain<=15; bucket e.g. fraction of reported limit); log when either gate trips
- [x] 2.7 Register the middleware in `backend/src/main.rs` on the `ClientBuilder` chain, after `TracingMiddleware`
- [x] 2.8 Unit tests: error gate (above/below threshold, 420 process-wide hard-stop, shared across callers); token bucket (per-`(group,userID)` isolation, below-threshold wait, 429 + `Retry-After` hard wait, cold-bucket passthrough); missing-both-headers leaves state unchanged; mutually-exclusive header sets handled
- [x] 2.9 Verify `token_sweep` and `esi::search` callers tolerate the new 420/429/unavailable outcome without panicking

## 3. Inbound: per-IP request rate limiter for /api/*

- [x] 3.1 Add a `GovernorLayer` (per-IP key) in `build_router` (`backend/src/lib.rs`), covering `/api/*`; choose the IP-extraction strategy against the Traefik forwarded-headers config
- [x] 3.2 Configure sustained rate + burst; make values config-driven via `Config` with conservative defaults
- [x] 3.3 Map limiter rejection to HTTP 429 with the project error envelope, `error.code = "rate_limited"`, and a `Retry-After` header (do not emit governor's default body)

## 4. Inbound: per-IP request rate limiter for /auth/*

- [x] 4.1 Add a second, separately-tuned (tighter) `GovernorLayer` covering `/auth/*` — at least `/auth/callback`; decide whether to include `login`/`add_character`. Reuse the same IP-extraction strategy as §3
- [x] 4.2 Make its rate + burst config-driven via `Config`, independent of the `/api/*` values
- [x] 4.3 On reject, redirect (302) to the dedicated "too busy" page — do NOT emit the `rate_limited` JSON envelope (api-contract exempts `/auth/*`). Define the backend redirect target; flag the frontend route as a follow-up
- [x] 4.4 Confirm rejected `/auth/callback` requests never reach the handler, so no SSO token exchange or outbound ESI call is made for them

## 5. api-contract: rate_limited error code

- [x] 5.1 Add `rate_limited` to the canonical error-code enum/mapping in the backend response/error module so the `/api/*` 429 path reuses the existing envelope
- [x] 5.2 Document the 429 / `rate_limited` response in `backend/src/openapi.rs` consistent with how shared codes are already published; ensure the strict OpenAPI test still passes

## 6. Integration + HURL tests

- [x] 6.1 Integration test: hammering an `/api/*` route past the limit yields HTTP 429 with the standard envelope, `error.code = "rate_limited"`, and a `Retry-After` header; under-limit requests are unaffected
- [x] 6.2 Integration test: hammering `/auth/callback` past the auth limit yields a 302 redirect to the too-busy page (NOT a JSON envelope) and the handler is not invoked
- [x] 6.3 Confirm per-IP isolation in both tests (distinct source keys throttle independently) to the extent the test harness allows
- [x] 6.4 Add HURL coverage under `backend/tests/hurl/` for both a throttled `/api/*` response and a throttled `/auth/callback` redirect

## 7. Verification

- [x] 7.1 `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean
- [x] 7.2 `cargo test` (unit + integration, incl. the OpenAPI strict test) green
- [x] 7.3 Run the HURL suite (mock + live where applicable) green — `tests/hurl/rate_limit.hurl` passes live (29 requests): /api 429 `rate_limited` envelope + Retry-After, /auth/callback 303 → /too-busy. Existing me/health HURL still green through the limiter.
- [x] 7.4 Manual sanity: live burst on `/api/v1/me` → 401s within burst then 429 + `Retry-After` + `{"error":{"code":"rate_limited",...}}`; `/auth/callback` burst → 303 /too-busy. Outbound gate verified healthy: a live entity search returned real ESI data through the middleware chain with no 420/429 or gate-trip warnings (warnings only fire on trip, by design).
