# Design — harden-auth-flow

## Context

The SSO flow stores in-flight OAuth state purely server-side: `/auth/login` writes an `InflightRecord` keyed by a UUID `csrf_state` into an in-memory `HashMap` (`session.rs`), redirects to EVE SSO, and `/auth/callback` consumes the record by the returned `state` query value. Nothing ties the callback to the browser that initiated the login, records are only removed on a completed callback, the session cookie is `HttpOnly; SameSite=Lax; Path=/` with no `Secure`, and logout is a GET route. There is currently no rate limiting (separate change `add-esi-rate-limit-backoff`, not yet applied), which makes the unbounded in-flight store independently abusable.

## Goals / Non-Goals

**Goals:**
- A callback only completes in the browser that started the flow.
- The in-flight store is bounded in size and time.
- Session and state cookies are not transmitted over plaintext HTTP in production.
- Logout cannot be triggered cross-site.

**Non-Goals:**
- PKCE. The backend is a confidential client (it holds `ESI_CLIENT_SECRET`); the state cookie closes the login-CSRF hole PKCE would also close. PKCE can be layered later without changing this design.
- Rate limiting (owned by `add-esi-rate-limit-backoff`).
- JWKS verification of ESI tokens or encryption-key separation (owned by `harden-token-crypto`).

## Decisions

**State cookie, not signed state.** `/auth/login` and `/auth/characters/add` set `auth_state=<csrf_state>` as `HttpOnly; SameSite=Lax; Secure; Path=/auth; Max-Age=900`. The callback requires `auth_state` to equal `query.state` *before* calling `InflightStore::take`, then clears the cookie on every outcome (success, blocked, error). Alternative considered: HMAC-signing the state with a browser nonce — more moving parts for the same binding guarantee; the cookie is the established pattern. `SameSite=Lax` is required (not `Strict`): the callback arrives as a top-level cross-site navigation from `login.eveonline.com`, which Lax permits for cookie *sending*.

**Path=/auth scope.** The state cookie is only meaningful to the auth routes; scoping it keeps it off every API request.

**TTL + cap inside `InflightStore`, not a background task.** `InflightRecord` gains `created_at: Instant`. `add()` first drops expired entries (single pass; the map is small in practice), then enforces a cap of 10 000 entries — beyond it, the new login is refused with a 503-style error rather than evicting live records (an attacker at the cap should not be able to evict legitimate in-flight logins; legitimate overflow at 10k concurrent logins is not a realistic state for this deployment). `take()` treats an expired record as absent. TTL is 15 minutes — generous for an SSO round-trip, matching the state cookie `Max-Age`. Alternative considered: a `tokio` interval sweeper — unnecessary machinery; opportunistic sweep on insert is enough because the map only grows via `add()`.

**`Secure` always on, dev handled by Traefik.** The dev compose stack already fronts the app with Traefik; dev runs over `https://` (or `localhost`, which browsers treat as a secure context and accept `Secure` cookies on). No config flag to disable `Secure` — a flag is a footgun that ends up set in production. If plain-HTTP dev turns out to matter, revisit with an explicit `COOKIE_SECURE=false` escape hatch then, not pre-emptively.

**Logout becomes POST only.** The GET route is removed, not kept as a deprecated alias — an alias would preserve the CSRF hole the change exists to close. The frontend replaces the logout `<a href="/auth/logout">` with a minimal `<form method="POST" action="/auth/logout">`. No CSRF token is needed beyond `SameSite=Lax`, which blocks cross-site POSTs.

## Risks / Trade-offs

- [Multiple login tabs] One browser starting logins in two tabs: the second tab's `Set-Cookie` overwrites the first's `auth_state`, so completing tab 1's callback fails the cookie match. → Acceptable: the user retries; this mirrors how most state-cookie implementations behave. The in-flight record remains until TTL, harmless.
- [Cookie cleared on error paths missed] If an error path forgets to clear `auth_state`, a stale cookie lingers 15 minutes. → It only ever has to *match* the in-flight state, so a stale value cannot be replayed; Max-Age bounds it regardless.
- [Cap refusal under attack] At the 10k cap, legitimate logins are refused alongside the attacker's. → Trade-off accepted: refusal is visible and recoverable; unbounded memory growth is not. The rate-limit change reduces how often the cap is reachable.
- [Breaking logout] Any bookmark/script doing GET `/auth/logout` breaks. → Returns 405; the frontend ships the form in the same change.

## Migration Plan

Single deploy; no data migration. Frontend and backend land together in this change (the logout method flip requires both). Rollback is a revert.
